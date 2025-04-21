// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title IShardable
 * @dev Interface for contracts that can be sharded across the FlashChain network
 * @notice This interface defines the core functionality required for sharding compatibility
 */
interface IShardable {
    // Events
    event ShardAssigned(uint256 indexed shardId, address indexed contractAddress);
    event ShardConfigurationUpdated(uint256 indexed shardId, bytes32 indexed configHash);
    event CrossShardMessageSent(uint256 indexed fromShard, uint256 indexed toShard, bytes32 indexed messageId);
    event CrossShardMessageReceived(uint256 indexed fromShard, uint256 indexed toShard, bytes32 indexed messageId);
    event ShardValidatorAdded(uint256 indexed shardId, address indexed validator);
    event ShardValidatorRemoved(uint256 indexed shardId, address indexed validator);
    event ShardStateUpdated(uint256 indexed shardId, bytes32 indexed stateRoot);
    event ShardPerformanceMetric(uint256 indexed shardId, uint256 timestamp, uint256 tps, uint256 latency);

    // Structs
    struct ShardConfiguration {
        uint256 shardId;
        uint256 maxValidators;
        uint256 minValidators;
        uint256 consensusThreshold;
        uint256 blockInterval;
        uint256 maxTransactionsPerBlock;
        bytes32 genesisStateRoot;
        mapping(address => bool) validators;
        bool isActive;
        uint256 lastUpdateTimestamp;
    }

    struct ShardMetadata {
        uint256 totalTransactions;
        uint256 averageTPS;
        uint256 peakTPS;
        uint256 averageLatency;
        uint256 uptime;
        uint256 validatorCount;
        bytes32 lastStateRoot;
        uint256 lastBlockTimestamp;
    }

    struct CrossShardMessage {
        bytes32 messageId;
        uint256 fromShard;
        uint256 toShard;
        address sender;
        address recipient;
        bytes payload;
        uint256 timestamp;
        bool isProcessed;
        bytes32 proofHash;
    }

    struct ValidatorInfo {
        address validatorAddress;
        uint256 stake;
        uint256 joinTimestamp;
        uint256 performanceScore;
        uint256 totalBlocksProduced;
        uint256 lastActiveTimestamp;
        bool isActive;
    }

    // Core Functions
    
    /**
     * @dev Initializes a new shard with the given configuration
     * @param shardId Unique identifier for the shard
     * @param config Initial configuration parameters for the shard
     * @return success Boolean indicating if initialization was successful
     */
    function initializeShard(uint256 shardId, ShardConfiguration calldata config) external returns (bool success);

    /**
     * @dev Updates the configuration of an existing shard
     * @param shardId ID of the shard to update
     * @param newConfig New configuration parameters
     * @return success Boolean indicating if update was successful
     */
    function updateShardConfiguration(uint256 shardId, ShardConfiguration calldata newConfig) external returns (bool success);

    /**
     * @dev Processes an incoming cross-shard message
     * @param message The cross-shard message to process
     * @return messageId The unique identifier of the processed message
     */
    function processCrossShardMessage(CrossShardMessage calldata message) external returns (bytes32 messageId);

    /**
     * @dev Sends a message to another shard
     * @param toShard Target shard ID
     * @param recipient Address of the recipient in the target shard
     * @param payload Message payload
     * @return messageId The unique identifier of the sent message
     */
    function sendCrossShardMessage(uint256 toShard, address recipient, bytes calldata payload) external returns (bytes32 messageId);

    // Validator Management Functions

    /**
     * @dev Adds a new validator to the shard
     * @param shardId ID of the shard
     * @param validator Address of the validator
     * @param stake Amount of tokens staked by the validator
     */
    function addShardValidator(uint256 shardId, address validator, uint256 stake) external returns (bool);

    /**
     * @dev Removes a validator from the shard
     * @param shardId ID of the shard
     * @param validator Address of the validator to remove
     */
    function removeShardValidator(uint256 shardId, address validator) external returns (bool);

    /**
     * @dev Updates validator performance metrics
     * @param shardId ID of the shard
     * @param validator Address of the validator
     * @param performanceData New performance data for the validator
     */
    function updateValidatorPerformance(uint256 shardId, address validator, ValidatorInfo calldata performanceData) external returns (bool);

    // State Management Functions

    /**
     * @dev Updates the state root of a shard
     * @param shardId ID of the shard
     * @param newStateRoot New state root hash
     * @param proof Proof of state transition validity
     */
    function updateShardState(uint256 shardId, bytes32 newStateRoot, bytes calldata proof) external returns (bool);

    /**
     * @dev Verifies a cross-shard transaction proof
     * @param messageId ID of the cross-shard message
     * @param proof Proof of transaction validity
     */
    function verifyCrossShardProof(bytes32 messageId, bytes calldata proof) external view returns (bool);

    // View Functions

    /**
     * @dev Retrieves the current configuration of a shard
     * @param shardId ID of the shard
     */
    function getShardConfiguration(uint256 shardId) external view returns (ShardConfiguration memory);

    /**
     * @dev Retrieves metadata about a shard's performance
     * @param shardId ID of the shard
     */
    function getShardMetadata(uint256 shardId) external view returns (ShardMetadata memory);

    /**
     * @dev Retrieves information about a validator
     * @param shardId ID of the shard
     * @param validator Address of the validator
     */
    function getValidatorInfo(uint256 shardId, address validator) external view returns (ValidatorInfo memory);

    /**
     * @dev Retrieves information about a cross-shard message
     * @param messageId ID of the message
     */
    function getCrossShardMessage(bytes32 messageId) external view returns (CrossShardMessage memory);

    /**
     * @dev Checks if an address is a valid validator for a shard
     * @param shardId ID of the shard
     * @param validator Address to check
     */
    function isValidShardValidator(uint256 shardId, address validator) external view returns (bool);

    /**
     * @dev Retrieves the current state root of a shard
     * @param shardId ID of the shard
     */
    function getShardStateRoot(uint256 shardId) external view returns (bytes32);

    /**
     * @dev Retrieves the total number of cross-shard messages processed
     * @param shardId ID of the shard
     */
    function getTotalCrossShardMessages(uint256 shardId) external view returns (uint256);
}