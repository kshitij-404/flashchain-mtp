// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "./BridgeCore.sol";

/**
 * @title ChannelManager
 * @dev Manages Lightning channels and their lifecycle
 */
contract ChannelManager is AccessControl, ReentrancyGuard, Pausable {
    using ECDSA for bytes32;

    // State variables
    BridgeCore public bridgeCore;
    
    mapping(bytes32 => Channel) public channels;
    mapping(bytes32 => HTLC) public htlcs;
    mapping(address => bytes32[]) public userChannels;
    
    // Structs
    struct Channel {
        address[] participants;
        uint256 capacity;
        ChannelStatus status;
        uint256 settleTimeout;
        uint256 closingBlock;
        bytes32 closingStateHash;
        mapping(address => uint256) balances;
        mapping(address => bool) hasConfirmed;
        uint256 numHTLCs;
        uint256 totalLocked;
    }

    struct HTLC {
        address sender;
        address recipient;
        uint256 amount;
        bytes32 hashLock;
        uint256 timelock;
        HTLCStatus status;
        bytes32 channelId;
    }

    enum ChannelStatus {
        None,
        Opening,
        Active,
        Closing,
        Closed,
        Disputed
    }

    enum HTLCStatus {
        None,
        Pending,
        Completed,
        Refunded,
        Expired
    }

    // Events
    event ChannelOpened(
        bytes32 indexed channelId,
        address[] participants,
        uint256 capacity
    );
    
    event ChannelClosed(
        bytes32 indexed channelId,
        bytes32 stateHash
    );
    
    event HTLCCreated(
        bytes32 indexed channelId,
        bytes32 indexed htlcId,
        address sender,
        address recipient,
        uint256 amount
    );
    
    event HTLCResolved(
        bytes32 indexed channelId,
        bytes32 indexed htlcId,
        HTLCStatus status
    );

    event BalanceUpdated(
        bytes32 indexed channelId,
        address indexed participant,
        uint256 newBalance
    );

    event DisputeRaised(
        bytes32 indexed channelId,
        address initiator,
        bytes32 disputedStateHash
    );

    // Constants
    uint256 public constant MINIMUM_CAPACITY = 0.01 ether;
    uint256 public constant MAXIMUM_HTLCS = 100;
    uint256 public constant DISPUTE_WINDOW = 7 days;

    constructor(address _bridgeCore) {
        bridgeCore = BridgeCore(_bridgeCore);
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    /**
     * @dev Opens a new payment channel
     * @param participants Array of channel participants
     * @param capacity Total channel capacity
     */
    function openChannel(
        address[] calldata participants,
        uint256 capacity
    ) 
        external 
        payable 
        nonReentrant 
        whenNotPaused 
        returns (bytes32) 
    {
        require(participants.length >= 2, "Minimum 2 participants required");
        require(capacity >= MINIMUM_CAPACITY, "Capacity too low");
        require(msg.value == capacity, "Incorrect funding amount");

        bytes32 channelId = keccak256(abi.encodePacked(
            block.timestamp,
            participants,
            capacity
        ));

        require(channels[channelId].status == ChannelStatus.None, "Channel exists");

        Channel storage channel = channels[channelId];
        channel.participants = participants;
        channel.capacity = capacity;
        channel.status = ChannelStatus.Opening;
        channel.settleTimeout = block.timestamp + DISPUTE_WINDOW;

        // Initialize balances
        uint256 initialBalance = capacity / participants.length;
        for (uint i = 0; i < participants.length; i++) {
            channel.balances[participants[i]] = initialBalance;
            userChannels[participants[i]].push(channelId);
        }

        // Register with bridge
        bridgeCore.registerChannel(participants, capacity);

        emit ChannelOpened(channelId, participants, capacity);
        return channelId;
    }

    /**
     * @dev Creates a new HTLC in the channel
     * @param channelId Channel identifier
     * @param recipient Recipient address
     * @param amount HTLC amount
     * @param hashLock Hash of the preimage
     * @param timelock Timelock for the HTLC
     */
    function createHTLC(
        bytes32 channelId,
        address recipient,
        uint256 amount,
        bytes32 hashLock,
        uint256 timelock
    ) 
        external 
        nonReentrant 
        returns (bytes32) 
    {
        Channel storage channel = channels[channelId];
        require(channel.status == ChannelStatus.Active, "Channel not active");
        require(channel.numHTLCs < MAXIMUM_HTLCS, "Too many HTLCs");
        require(channel.balances[msg.sender] >= amount, "Insufficient balance");

        bytes32 htlcId = keccak256(abi.encodePacked(
            channelId,
            msg.sender,
            recipient,
            amount,
            hashLock,
            timelock
        ));

        HTLC storage htlc = htlcs[htlcId];
        require(htlc.status == HTLCStatus.None, "HTLC exists");

        htlc.sender = msg.sender;
        htlc.recipient = recipient;
        htlc.amount = amount;
        htlc.hashLock = hashLock;
        htlc.timelock = timelock;
        htlc.status = HTLCStatus.Pending;
        htlc.channelId = channelId;

        channel.balances[msg.sender] -= amount;
        channel.totalLocked += amount;
        channel.numHTLCs++;

        emit HTLCCreated(channelId, htlcId, msg.sender, recipient, amount);
        return htlcId;
    }

    /**
     * @dev Resolves an HTLC with the preimage
     * @param htlcId HTLC identifier
     * @param preimage Preimage of the hashlock
     */
    function resolveHTLC(bytes32 htlcId, bytes32 preimage) 
        external 
        nonReentrant 
    {
        HTLC storage htlc = htlcs[htlcId];
        require(htlc.status == HTLCStatus.Pending, "HTLC not pending");
        require(keccak256(abi.encodePacked(preimage)) == htlc.hashLock, "Invalid preimage");
        require(block.timestamp < htlc.timelock, "HTLC expired");

        Channel storage channel = channels[htlc.channelId];
        channel.balances[htlc.recipient] += htlc.amount;
        channel.totalLocked -= htlc.amount;
        channel.numHTLCs--;

        htlc.status = HTLCStatus.Completed;

        emit HTLCResolved(htlc.channelId, htlcId, HTLCStatus.Completed);
        emit BalanceUpdated(htlc.channelId, htlc.recipient, channel.balances[htlc.recipient]);
    }

    /**
     * @dev Initiates channel closure
     * @param channelId Channel identifier
     * @param stateHash Hash of the final state
     */
    function initiateClose(bytes32 channelId, bytes32 stateHash) 
        external 
        nonReentrant 
    {
        Channel storage channel = channels[channelId];
        require(channel.status == ChannelStatus.Active, "Channel not active");
        require(_isParticipant(channelId, msg.sender), "Not a participant");

        channel.status = ChannelStatus.Closing;
        channel.closingBlock = block.number;
        channel.closingStateHash = stateHash;

        emit ChannelClosed(channelId, stateHash);
    }

    /**
     * @dev Confirms channel closure
     * @param channelId Channel identifier
     */
    function confirmClose(bytes32 channelId) 
        external 
        nonReentrant 
    {
        Channel storage channel = channels[channelId];
        require(channel.status == ChannelStatus.Closing, "Channel not closing");
        require(_isParticipant(channelId, msg.sender), "Not a participant");
        require(!channel.hasConfirmed[msg.sender], "Already confirmed");

        channel.hasConfirmed[msg.sender] = true;

        // Check if all participants have confirmed
        bool allConfirmed = true;
        for (uint i = 0; i < channel.participants.length; i++) {
            if (!channel.hasConfirmed[channel.participants[i]]) {
                allConfirmed = false;
                break;
            }
        }

        if (allConfirmed) {
            _settleChannel(channelId);
        }
    }

    /**
     * @dev Raises a dispute for the channel
     * @param channelId Channel identifier
     * @param disputedStateHash Hash of the disputed state
     */
    function raiseDispute(bytes32 channelId, bytes32 disputedStateHash) 
        external 
        nonReentrant 
    {
        Channel storage channel = channels[channelId];
        require(channel.status == ChannelStatus.Closing, "Channel not closing");
        require(_isParticipant(channelId, msg.sender), "Not a participant");

        channel.status = ChannelStatus.Disputed;
        
        // Notify bridge of dispute
        bridgeCore.initiateDispute(channelId, "");

        emit DisputeRaised(channelId, msg.sender, disputedStateHash);
    }

    // Internal functions

    function _isParticipant(bytes32 channelId, address participant) 
        internal 
        view 
        returns (bool) 
    {
        Channel storage channel = channels[channelId];
        for (uint i = 0; i < channel.participants.length; i++) {
            if (channel.participants[i] == participant) {
                return true;
            }
        }
        return false;
    }

    function _settleChannel(bytes32 channelId) internal {
        Channel storage channel = channels[channelId];
        
        // Distribute funds according to final balances
        for (uint i = 0; i < channel.participants.length; i++) {
            address participant = channel.participants[i];
            uint256 balance = channel.balances[participant];
            if (balance > 0) {
                payable(participant).transfer(balance);
            }
        }

        channel.status = ChannelStatus.Closed;
    }

    // View functions

    function getChannel(bytes32 channelId) 
        external 
        view 
        returns (
            address[] memory participants,
            uint256 capacity,
            ChannelStatus status,
            uint256 settleTimeout,
            uint256 closingBlock,
            bytes32 closingStateHash,
            uint256 numHTLCs,
            uint256 totalLocked
        ) 
    {
        Channel storage channel = channels[channelId];
        return (
            channel.participants,
            channel.capacity,
            channel.status,
            channel.settleTimeout,
            channel.closingBlock,
            channel.closingStateHash,
            channel.numHTLCs,
            channel.totalLocked
        );
    }

    function getBalance(bytes32 channelId, address participant) 
        external 
        view 
        returns (uint256) 
    {
        return channels[channelId].balances[participant];
    }

    function getUserChannels(address user) 
        external 
        view 
        returns (bytes32[] memory) 
    {
        return userChannels[user];
    }

    // Admin functions

    function pause() 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        _pause();
    }

    function unpause() 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        _unpause();
    }

    // Fallback and receive functions
    receive() external payable {}
    fallback() external payable {}
}