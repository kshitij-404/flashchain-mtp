// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";

/**
 * @title BridgeCore
 * @dev Core contract for bridging between base layer and lightning layer
 */
contract BridgeCore is AccessControl, ReentrancyGuard, Pausable {
    using ECDSA for bytes32;

    bytes32 public constant BRIDGE_ADMIN_ROLE = keccak256("BRIDGE_ADMIN_ROLE");
    bytes32 public constant VALIDATOR_ROLE = keccak256("VALIDATOR_ROLE");
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");

    // Events
    event ChannelRegistered(bytes32 indexed channelId, address[] participants);
    event ChannelStateUpdated(bytes32 indexed channelId, bytes32 stateHash);
    event DisputeInitiated(bytes32 indexed channelId, address initiator);
    event DisputeResolved(bytes32 indexed channelId, bytes32 finalStateHash);
    event FundsLocked(bytes32 indexed channelId, uint256 amount);
    event FundsReleased(bytes32 indexed channelId, uint256 amount);
    event ValidatorAdded(address indexed validator);
    event ValidatorRemoved(address indexed validator);

    // Structs
    struct Channel {
        address[] participants;
        uint256 capacity;
        uint256 lockedFunds;
        bytes32 latestStateHash;
        uint256 disputePeriodEnd;
        bool isActive;
        mapping(address => bool) hasConsented;
        DisputeStatus disputeStatus;
    }

    struct ChannelState {
        bytes32 channelId;
        uint256 sequence;
        mapping(address => uint256) balances;
        mapping(bytes32 => HTLC) htlcs;
        uint256 timestamp;
    }

    struct HTLC {
        address sender;
        address recipient;
        uint256 amount;
        bytes32 hashLock;
        uint256 timelock;
        bool isSettled;
    }

    enum DisputeStatus {
        None,
        Initiated,
        Resolved
    }

    // State variables
    mapping(bytes32 => Channel) public channels;
    mapping(bytes32 => ChannelState) public channelStates;
    mapping(address => uint256) public validatorStakes;
    
    uint256 public constant MINIMUM_STAKE = 1000 ether;
    uint256 public constant DISPUTE_PERIOD = 7 days;
    uint256 public constant MAX_CHANNELS_PER_PARTICIPANT = 100;

    constructor() {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(BRIDGE_ADMIN_ROLE, msg.sender);
    }

    /**
     * @dev Registers a new channel
     * @param participants Array of channel participants
     * @param capacity Total channel capacity
     */
    function registerChannel(
        address[] calldata participants,
        uint256 capacity
    ) 
        external 
        nonReentrant 
        whenNotPaused 
        returns (bytes32) 
    {
        require(participants.length >= 2, "Minimum 2 participants required");
        require(capacity > 0, "Capacity must be positive");

        bytes32 channelId = keccak256(abi.encodePacked(
            block.timestamp,
            participants,
            capacity
        ));

        require(!channels[channelId].isActive, "Channel already exists");

        Channel storage channel = channels[channelId];
        channel.participants = participants;
        channel.capacity = capacity;
        channel.isActive = true;

        emit ChannelRegistered(channelId, participants);
        return channelId;
    }

    /**
     * @dev Updates channel state with signatures from all participants
     * @param channelId Channel identifier
     * @param stateHash Hash of the new state
     * @param signatures Array of signatures from participants
     */
    function updateChannelState(
        bytes32 channelId,
        bytes32 stateHash,
        bytes[] calldata signatures
    ) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        Channel storage channel = channels[channelId];
        require(channel.isActive, "Channel not active");
        require(signatures.length == channel.participants.length, "Invalid signature count");

        // Verify all signatures
        bytes32 messageHash = keccak256(abi.encodePacked(channelId, stateHash));
        for (uint i = 0; i < signatures.length; i++) {
            address signer = messageHash.toEthSignedMessageHash().recover(signatures[i]);
            require(_isParticipant(channelId, signer), "Invalid signature");
        }

        channel.latestStateHash = stateHash;
        emit ChannelStateUpdated(channelId, stateHash);
    }

    /**
     * @dev Initiates a dispute for a channel
     * @param channelId Channel identifier
     * @param stateProof Proof of the latest valid state
     */
    function initiateDispute(
        bytes32 channelId,
        bytes calldata stateProof
    ) 
        external 
        nonReentrant 
    {
        Channel storage channel = channels[channelId];
        require(channel.isActive, "Channel not active");
        require(_isParticipant(channelId, msg.sender), "Not a participant");
        require(channel.disputeStatus == DisputeStatus.None, "Dispute already exists");

        channel.disputeStatus = DisputeStatus.Initiated;
        channel.disputePeriodEnd = block.timestamp + DISPUTE_PERIOD;

        emit DisputeInitiated(channelId, msg.sender);
    }

    /**
     * @dev Resolves a dispute with validator consensus
     * @param channelId Channel identifier
     * @param finalStateHash Hash of the final state
     * @param validatorSignatures Array of validator signatures
     */
    function resolveDispute(
        bytes32 channelId,
        bytes32 finalStateHash,
        bytes[] calldata validatorSignatures
    ) 
        external 
        nonReentrant 
    {
        Channel storage channel = channels[channelId];
        require(channel.disputeStatus == DisputeStatus.Initiated, "No active dispute");
        require(block.timestamp > channel.disputePeriodEnd, "Dispute period not ended");

        // Verify validator signatures
        uint256 validSignatures = 0;
        bytes32 messageHash = keccak256(abi.encodePacked(channelId, finalStateHash));
        
        for (uint i = 0; i < validatorSignatures.length; i++) {
            address validator = messageHash.toEthSignedMessageHash().recover(validatorSignatures[i]);
            if (hasRole(VALIDATOR_ROLE, validator)) {
                validSignatures++;
            }
        }

        require(validSignatures >= _requiredValidatorCount(), "Insufficient validator signatures");

        channel.latestStateHash = finalStateHash;
        channel.disputeStatus = DisputeStatus.Resolved;

        emit DisputeResolved(channelId, finalStateHash);
    }

    /**
     * @dev Locks funds in the channel
     * @param channelId Channel identifier
     */
    function lockFunds(bytes32 channelId) 
        external 
        payable 
        nonReentrant 
        whenNotPaused 
    {
        Channel storage channel = channels[channelId];
        require(channel.isActive, "Channel not active");
        require(_isParticipant(channelId, msg.sender), "Not a participant");
        require(channel.lockedFunds + msg.value <= channel.capacity, "Exceeds capacity");

        channel.lockedFunds += msg.value;
        emit FundsLocked(channelId, msg.value);
    }

    /**
     * @dev Releases funds from the channel
     * @param channelId Channel identifier
     * @param amount Amount to release
     * @param recipient Recipient address
     */
    function releaseFunds(
        bytes32 channelId,
        uint256 amount,
        address payable recipient
    ) 
        external 
        nonReentrant 
    {
        Channel storage channel = channels[channelId];
        require(channel.isActive, "Channel not active");
        require(_isParticipant(channelId, msg.sender), "Not a participant");
        require(channel.lockedFunds >= amount, "Insufficient funds");

        channel.lockedFunds -= amount;
        (bool success, ) = recipient.call{value: amount}("");
        require(success, "Transfer failed");

        emit FundsReleased(channelId, amount);
    }

    /**
     * @dev Adds a new validator
     * @param validator Address of the validator
     */
    function addValidator(address validator) 
        external 
        onlyRole(BRIDGE_ADMIN_ROLE) 
    {
        require(validator != address(0), "Invalid address");
        grantRole(VALIDATOR_ROLE, validator);
        emit ValidatorAdded(validator);
    }

    /**
     * @dev Removes a validator
     * @param validator Address of the validator
     */
    function removeValidator(address validator) 
        external 
        onlyRole(BRIDGE_ADMIN_ROLE) 
    {
        revokeRole(VALIDATOR_ROLE, validator);
        emit ValidatorRemoved(validator);
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

    function _requiredValidatorCount() 
        internal 
        view 
        returns (uint256) 
    {
        uint256 totalValidators = getRoleMemberCount(VALIDATOR_ROLE);
        return (totalValidators * 2) / 3 + 1; // 2/3 + 1 majority
    }

    // Emergency functions

    function pause() 
        external 
        onlyRole(BRIDGE_ADMIN_ROLE) 
    {
        _pause();
    }

    function unpause() 
        external 
        onlyRole(BRIDGE_ADMIN_ROLE) 
    {
        _unpause();
    }

    // View functions

    function getChannel(bytes32 channelId) 
        external 
        view 
        returns (
            address[] memory participants,
            uint256 capacity,
            uint256 lockedFunds,
            bytes32 latestStateHash,
            bool isActive,
            DisputeStatus disputeStatus
        ) 
    {
        Channel storage channel = channels[channelId];
        return (
            channel.participants,
            channel.capacity,
            channel.lockedFunds,
            channel.latestStateHash,
            channel.isActive,
            channel.disputeStatus
        );
    }

    function getParticipantChannels(address participant) 
        external 
        view 
        returns (bytes32[] memory) 
    {
        bytes32[] memory result = new bytes32[](MAX_CHANNELS_PER_PARTICIPANT);
        uint256 count = 0;

        // This is not gas-efficient for large numbers of channels
        // In production, maintain a mapping of participant -> channels
        for (uint i = 0; count < MAX_CHANNELS_PER_PARTICIPANT; i++) {
            bytes32 channelId = bytes32(i);
            if (_isParticipant(channelId, participant)) {
                result[count] = channelId;
                count++;
            }
        }

        // Trim array to actual size
        bytes32[] memory trimmedResult = new bytes32[](count);
        for (uint i = 0; i < count; i++) {
            trimmedResult[i] = result[i];
        }

        return trimmedResult;
    }
}