// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "./IShardable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/Counters.sol";

/**
 * @title BaseShardContract
 * @dev Abstract base contract implementing core sharding functionality
 */
abstract contract BaseShardContract is IShardable, AccessControl, ReentrancyGuard, Pausable {
    using Counters for Counters.Counter;
    using ECDSA for bytes32;

    // Role definitions
    bytes32 public constant SHARD_ADMIN_ROLE = keccak256("SHARD_ADMIN_ROLE");
    bytes32 public constant VALIDATOR_ROLE = keccak256("VALIDATOR_ROLE");
    bytes32 public constant BRIDGE_ROLE = keccak256("BRIDGE_ROLE");

    // State variables
    mapping(uint256 => ShardConfiguration) private _shardConfigurations;
    mapping(uint256 => ShardMetadata) private _shardMetadata;
    mapping(uint256 => mapping(address => ValidatorInfo)) private _validatorInfo;
    mapping(bytes32 => CrossShardMessage) private _crossShardMessages;
    mapping(uint256 => bytes32[]) private _shardStateHistory;
    
    Counters.Counter private _messageIdCounter;
    
    // Constants
    uint256 private constant MAX_SHARD_COUNT = 1024;
    uint256 private constant MIN_VALIDATOR_STAKE = 1000 ether;
    uint256 private constant PERFORMANCE_THRESHOLD = 95;
    uint256 private constant MAX_MESSAGE_SIZE = 1024 * 1024; // 1MB

    // Modifiers
    modifier validShardId(uint256 shardId) {
        require(shardId < MAX_SHARD_COUNT, "BaseShardContract: Invalid shard ID");
        require(_shardConfigurations[shardId].isActive, "BaseShardContract: Shard not active");
        _;
    }

    modifier onlyValidator(uint256 shardId) {
        require(
            hasRole(VALIDATOR_ROLE, msg.sender) && 
            _validatorInfo[shardId][msg.sender].isActive,
            "BaseShardContract: Caller is not an active validator"
        );
        _;
    }

    modifier onlyShardAdmin() {
        require(hasRole(SHARD_ADMIN_ROLE, msg.sender), "BaseShardContract: Caller is not a shard admin");
        _;
    }

    modifier validMessageSize(bytes calldata payload) {
        require(payload.length <= MAX_MESSAGE_SIZE, "BaseShardContract: Message too large");
        _;
    }

    // Constructor
    constructor() {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(SHARD_ADMIN_ROLE, msg.sender);
    }

    // Implementation of IShardable interface functions

    function initializeShard(uint256 shardId, ShardConfiguration calldata config) 
        external 
        override 
        onlyShardAdmin 
        returns (bool) 
    {
        require(!_shardConfigurations[shardId].isActive, "BaseShardContract: Shard already initialized");
        require(config.minValidators > 0, "BaseShardContract: Invalid min validators");
        require(config.maxValidators >= config.minValidators, "BaseShardContract: Invalid max validators");
        require(config.consensusThreshold > 0, "BaseShardContract: Invalid consensus threshold");

        _shardConfigurations[shardId] = config;
        _shardConfigurations[shardId].isActive = true;
        _shardConfigurations[shardId].lastUpdateTimestamp = block.timestamp;

        _initializeShardMetadata(shardId);

        emit ShardAssigned(shardId, address(this));
        return true;
    }

    function updateShardConfiguration(uint256 shardId, ShardConfiguration calldata newConfig) 
        external 
        override 
        onlyShardAdmin 
        validShardId(shardId) 
        returns (bool) 
    {
        require(newConfig.minValidators > 0, "BaseShardContract: Invalid min validators");
        require(newConfig.maxValidators >= newConfig.minValidators, "BaseShardContract: Invalid max validators");

        _shardConfigurations[shardId] = newConfig;
        _shardConfigurations[shardId].lastUpdateTimestamp = block.timestamp;

        bytes32 configHash = keccak256(abi.encode(newConfig));
        emit ShardConfigurationUpdated(shardId, configHash);
        return true;
    }

    function processCrossShardMessage(CrossShardMessage calldata message) 
        external 
        override 
        nonReentrant 
        validShardId(message.toShard) 
        returns (bytes32) 
    {
        require(!_crossShardMessages[message.messageId].isProcessed, "BaseShardContract: Message already processed");
        require(_validateCrossShardMessage(message), "BaseShardContract: Invalid message");

        _crossShardMessages[message.messageId] = message;
        _crossShardMessages[message.messageId].isProcessed = true;

        emit CrossShardMessageReceived(message.fromShard, message.toShard, message.messageId);
        return message.messageId;
    }

    function sendCrossShardMessage(
        uint256 toShard, 
        address recipient, 
        bytes calldata payload
    ) 
        external 
        override 
        nonReentrant 
        validShardId(toShard) 
        validMessageSize(payload) 
        returns (bytes32) 
    {
        _messageIdCounter.increment();
        bytes32 messageId = keccak256(abi.encodePacked(
            block.timestamp,
            msg.sender,
            toShard,
            recipient,
            payload,
            _messageIdCounter.current()
        ));

        CrossShardMessage memory message = CrossShardMessage({
            messageId: messageId,
            fromShard: _getCurrentShardId(),
            toShard: toShard,
            sender: msg.sender,
            recipient: recipient,
            payload: payload,
            timestamp: block.timestamp,
            isProcessed: false,
            proofHash: bytes32(0)
        });

        _crossShardMessages[messageId] = message;
        emit CrossShardMessageSent(message.fromShard, toShard, messageId);
        return messageId;
    }

    // Internal utility functions

    function _initializeShardMetadata(uint256 shardId) internal {
        _shardMetadata[shardId] = ShardMetadata({
            totalTransactions: 0,
            averageTPS: 0,
            peakTPS: 0,
            averageLatency: 0,
            uptime: block.timestamp,
            validatorCount: 0,
            lastStateRoot: bytes32(0),
            lastBlockTimestamp: block.timestamp
        });
    }

    function _validateCrossShardMessage(CrossShardMessage calldata message) 
        internal 
        view 
        returns (bool) 
    {
        // Add custom validation logic here
        return true;
    }

    function _getCurrentShardId() internal view virtual returns (uint256) {
        // Implementation should be provided by derived contracts
        revert("BaseShardContract: Not implemented");
    }

    // Additional helper functions can be added here

    // Function to handle contract upgrades and migrations
    function _migrate(address newImplementation) internal onlyShardAdmin {
        // Add migration logic here
    }

    // Emergency functions
    function pause() external onlyShardAdmin {
        _pause();
    }

    function unpause() external onlyShardAdmin {
        _unpause();
    }

    // Required overrides for child contracts
    function updateShardState(uint256 shardId, bytes32 newStateRoot, bytes calldata proof) 
        external 
        virtual 
        override 
        returns (bool);

    function verifyCrossShardProof(bytes32 messageId, bytes calldata proof) 
        external 
        virtual 
        override 
        view 
        returns (bool);

    // View functions implementation
    function getShardConfiguration(uint256 shardId) 
        external 
        view 
        override 
        validShardId(shardId) 
        returns (ShardConfiguration memory) 
    {
        return _shardConfigurations[shardId];
    }

    function getShardMetadata(uint256 shardId) 
        external 
        view 
        override 
        validShardId(shardId) 
        returns (ShardMetadata memory) 
    {
        return _shardMetadata[shardId];
    }

    function getCrossShardMessage(bytes32 messageId) 
        external 
        view 
        override 
        returns (CrossShardMessage memory) 
    {
        return _crossShardMessages[messageId];
    }

    // Add any additional custom functionality here
}