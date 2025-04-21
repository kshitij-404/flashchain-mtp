// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../base/BaseShardContract.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/**
 * @title ShardRegistry
 * @dev Manages registration and tracking of shards in the FlashChain network
 */
contract ShardRegistry is BaseShardContract, ReentrancyGuard {
    using SafeMath for uint256;
    using EnumerableSet for EnumerableSet.AddressSet;
    using EnumerableSet for EnumerableSet.UintSet;
    using EnumerableSet for EnumerableSet.Bytes32Set;

    // Structs
    struct ShardRegistration {
        uint256 shardId;
        address contractAddress;
        bytes32 genesisHash;
        uint256 registrationTime;
        uint256 lastUpdateTime;
        ShardType shardType;
        RegistrationStatus status;
        mapping(address => bool) authorizedUpdaters;
        bytes32[] stateRootHistory;
    }

    struct ShardMetadata {
        string name;
        string description;
        string version;
        bytes32 configHash;
        address[] operators;
        mapping(bytes32 => bool) supportedProtocols;
    }

    struct RegistrationRequest {
        uint256 requestId;
        address requester;
        uint256 timestamp;
        bytes32 configHash;
        RequestStatus status;
        string reason;
    }

    struct StateUpdate {
        bytes32 previousStateRoot;
        bytes32 newStateRoot;
        uint256 blockNumber;
        uint256 timestamp;
        address updater;
        bytes proof;
    }

    // Enums
    enum ShardType {
        Standard,
        Specialized,
        Archive,
        Lightweight,
        Testing
    }

    enum RegistrationStatus {
        Unregistered,
        Pending,
        Active,
        Suspended,
        Deprecated
    }

    enum RequestStatus {
        Pending,
        Approved,
        Rejected,
        Cancelled
    }

    // Events
    event ShardRegistered(uint256 indexed shardId, address indexed contractAddress);
    event ShardStatusUpdated(uint256 indexed shardId, RegistrationStatus status);
    event StateRootUpdated(uint256 indexed shardId, bytes32 stateRoot);
    event RegistrationRequested(uint256 indexed requestId, address indexed requester);
    event UpdaterAuthorized(uint256 indexed shardId, address indexed updater);
    event UpdaterRevoked(uint256 indexed shardId, address indexed updater);
    event MetadataUpdated(uint256 indexed shardId, bytes32 configHash);
    event ProtocolSupported(uint256 indexed shardId, bytes32 protocol);
    event EmergencyFreeze(uint256 indexed shardId, string reason);

    // State variables
    mapping(uint256 => ShardRegistration) public registrations;
    mapping(uint256 => ShardMetadata) public metadata;
    mapping(uint256 => mapping(uint256 => StateUpdate)) public stateUpdates;
    mapping(uint256 => RegistrationRequest) public registrationRequests;
    
    EnumerableSet.UintSet private registeredShards;
    EnumerableSet.Bytes32Set private supportedProtocols;
    
    // Constants
    uint256 public constant MAX_STATE_HISTORY = 1000;
    uint256 public constant MIN_OPERATORS = 3;
    uint256 public constant REGISTRATION_TIMEOUT = 7 days;
    uint256 public constant UPDATE_COOLDOWN = 1 hours;

    // Modifiers
    modifier validShardId(uint256 shardId) {
        require(registeredShards.contains(shardId), "ShardRegistry: Invalid shard ID");
        _;
    }

    modifier onlyAuthorizedUpdater(uint256 shardId) {
        require(registrations[shardId].authorizedUpdaters[msg.sender], 
                "ShardRegistry: Unauthorized updater");
        _;
    }

    /**
     * @dev Requests registration of a new shard
     * @param shardType Type of the shard
     * @param configHash Hash of shard configuration
     * @param metadata Initial metadata for the shard
     */
    function requestRegistration(
        ShardType shardType,
        bytes32 configHash,
        string calldata metadata
    ) 
        external 
        nonReentrant 
        returns (uint256) 
    {
        uint256 requestId = uint256(keccak256(abi.encodePacked(
            block.timestamp,
            msg.sender,
            configHash
        )));

        registrationRequests[requestId] = RegistrationRequest({
            requestId: requestId,
            requester: msg.sender,
            timestamp: block.timestamp,
            configHash: configHash,
            status: RequestStatus.Pending,
            reason: ""
        });

        emit RegistrationRequested(requestId, msg.sender);
        return requestId;
    }

    /**
     * @dev Registers a new shard after approval
     * @param requestId Registration request ID
     * @param contractAddress Address of the shard contract
     * @param genesisHash Genesis state hash of the shard
     */
    function registerShard(
        uint256 requestId,
        address contractAddress,
        bytes32 genesisHash
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        nonReentrant 
        returns (uint256 shardId) 
    {
        require(registrationRequests[requestId].status == RequestStatus.Pending, 
                "ShardRegistry: Invalid request");
        
        shardId = _generateShardId(contractAddress, genesisHash);
        require(!registeredShards.contains(shardId), "ShardRegistry: Already registered");

        ShardRegistration storage registration = registrations[shardId];
        registration.shardId = shardId;
        registration.contractAddress = contractAddress;
        registration.genesisHash = genesisHash;
        registration.registrationTime = block.timestamp;
        registration.lastUpdateTime = block.timestamp;
        registration.status = RegistrationStatus.Active;
        registration.stateRootHistory.push(genesisHash);

        registeredShards.add(shardId);
        registrationRequests[requestId].status = RequestStatus.Approved;

        emit ShardRegistered(shardId, contractAddress);
        return shardId;
    }

    /**
     * @dev Updates shard state root
     * @param shardId The shard ID
     * @param newStateRoot New state root hash
     * @param proof Proof of state transition
     */
    function updateStateRoot(
        uint256 shardId,
        bytes32 newStateRoot,
        bytes calldata proof
    ) 
        external 
        validShardId(shardId) 
        onlyAuthorizedUpdater(shardId) 
        nonReentrant 
    {
        require(block.timestamp >= registrations[shardId].lastUpdateTime.add(UPDATE_COOLDOWN), 
                "ShardRegistry: Update too frequent");

        ShardRegistration storage registration = registrations[shardId];
        bytes32 previousStateRoot = registration.stateRootHistory[
            registration.stateRootHistory.length - 1
        ];

        // Store state update
        uint256 updateIndex = block.number;
        stateUpdates[shardId][updateIndex] = StateUpdate({
            previousStateRoot: previousStateRoot,
            newStateRoot: newStateRoot,
            blockNumber: block.number,
            timestamp: block.timestamp,
            updater: msg.sender,
            proof: proof
        });

        // Update state root history
        registration.stateRootHistory.push(newStateRoot);
        if (registration.stateRootHistory.length > MAX_STATE_HISTORY) {
            _pruneStateHistory(shardId);
        }

        registration.lastUpdateTime = block.timestamp;
        emit StateRootUpdated(shardId, newStateRoot);
    }

    /**
     * @dev Authorizes an updater for a shard
     * @param shardId The shard ID
     * @param updater Address to authorize
     */
    function authorizeUpdater(
        uint256 shardId,
        address updater
    ) 
        external 
        validShardId(shardId) 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(updater != address(0), "ShardRegistry: Invalid updater");
        registrations[shardId].authorizedUpdaters[updater] = true;
        emit UpdaterAuthorized(shardId, updater);
    }

    /**
     * @dev Updates shard metadata
     * @param shardId The shard ID
     * @param name New name
     * @param description New description
     * @param version New version
     */
    function updateMetadata(
        uint256 shardId,
        string calldata name,
        string calldata description,
        string calldata version
    ) 
        external 
        validShardId(shardId) 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        ShardMetadata storage meta = metadata[shardId];
        meta.name = name;
        meta.description = description;
        meta.version = version;
        meta.configHash = keccak256(abi.encodePacked(name, description, version));

        emit MetadataUpdated(shardId, meta.configHash);
    }

    /**
     * @dev Adds supported protocol for a shard
     * @param shardId The shard ID
     * @param protocol Protocol identifier
     */
    function addSupportedProtocol(
        uint256 shardId,
        bytes32 protocol
    ) 
        external 
        validShardId(shardId) 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        metadata[shardId].supportedProtocols[protocol] = true;
        supportedProtocols.add(protocol);
        emit ProtocolSupported(shardId, protocol);
    }

    /**
     * @dev Freezes a shard in emergency
     * @param shardId The shard ID
     * @param reason Reason for freeze
     */
    function emergencyFreeze(
        uint256 shardId,
        string calldata reason
    ) 
        external 
        validShardId(shardId) 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        registrations[shardId].status = RegistrationStatus.Suspended;
        emit EmergencyFreeze(shardId, reason);
    }

    // Internal functions

    function _generateShardId(address contractAddress, bytes32 genesisHash) 
        internal 
        pure 
        returns (uint256) 
    {
        return uint256(keccak256(abi.encodePacked(contractAddress, genesisHash)));
    }

    function _pruneStateHistory(uint256 shardId) internal {
        ShardRegistration storage registration = registrations[shardId];
        uint256 historyLength = registration.stateRootHistory.length;
        uint256 startIndex = historyLength.sub(MAX_STATE_HISTORY);
        
        bytes32[] memory newHistory = new bytes32[](MAX_STATE_HISTORY);
        for (uint256 i = 0; i < MAX_STATE_HISTORY; i++) {
            newHistory[i] = registration.stateRootHistory[startIndex + i];
        }
        registration.stateRootHistory = newHistory;
    }

    // View functions

    function getRegistration(uint256 shardId) 
        external 
        view 
        returns (
            address contractAddress,
            bytes32 genesisHash,
            uint256 registrationTime,
            uint256 lastUpdateTime,
            ShardType shardType,
            RegistrationStatus status
        ) 
    {
        ShardRegistration storage registration = registrations[shardId];
        return (
            registration.contractAddress,
            registration.genesisHash,
            registration.registrationTime,
            registration.lastUpdateTime,
            registration.shardType,
            registration.status
        );
    }

    function getStateRootHistory(uint256 shardId) 
        external 
        view 
        returns (bytes32[] memory) 
    {
        return registrations[shardId].stateRootHistory;
    }

    function isAuthorizedUpdater(uint256 shardId, address updater) 
        external 
        view 
        returns (bool) 
    {
        return registrations[shardId].authorizedUpdaters[updater];
    }

    function getRegisteredShards() 
        external 
        view 
        returns (uint256[] memory) 
    {
        return registeredShards.values();
    }

    function getSupportedProtocols() 
        external 
        view 
        returns (bytes32[] memory) 
    {
        return supportedProtocols.values();
    }
}