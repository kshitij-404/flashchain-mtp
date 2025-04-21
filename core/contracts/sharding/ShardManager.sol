// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../base/BaseShardContract.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/**
 * @title ShardManager
 * @dev Manages shard creation, monitoring, and rebalancing in the FlashChain network
 */
contract ShardManager is BaseShardContract, ReentrancyGuard {
    using SafeMath for uint256;
    using EnumerableSet for EnumerableSet.AddressSet;
    using EnumerableSet for EnumerableSet.UintSet;

    // Structs
    struct ShardInfo {
        uint256 shardId;
        uint256 capacity;
        uint256 currentLoad;
        uint256 creationTime;
        uint256 lastRebalanceTime;
        bytes32 stateRoot;
        ShardStatus status;
        address[] validators;
        mapping(uint256 => uint256) epochStats;
        mapping(bytes32 => bool) processedCrossShardTxs;
    }

    struct ShardStats {
        uint256 totalTransactions;
        uint256 crossShardTransactions;
        uint256 averageLatency;
        uint256 peakTPS;
        uint256 failedTransactions;
        uint256 lastUpdateTimestamp;
    }

    struct RebalanceParams {
        uint256 threshold;
        uint256 cooldownPeriod;
        uint256 maxNodesPerMove;
        bool autoRebalance;
    }

    struct CrossShardRoute {
        uint256 sourceShardId;
        uint256 targetShardId;
        uint256 latency;
        uint256 bandwidth;
        bool isActive;
    }

    // Enums
    enum ShardStatus {
        Inactive,
        Initializing,
        Active,
        Degraded,
        Rebalancing,
        Maintenance
    }

    // Events
    event ShardCreated(uint256 indexed shardId, uint256 capacity);
    event ShardStatusUpdated(uint256 indexed shardId, ShardStatus status);
    event ValidatorAssigned(uint256 indexed shardId, address indexed validator);
    event ValidatorRemoved(uint256 indexed shardId, address indexed validator);
    event ShardRebalanced(uint256 indexed sourceShardId, uint256 indexed targetShardId);
    event CrossShardRouteEstablished(uint256 indexed sourceShardId, uint256 indexed targetShardId);
    event ShardLoadUpdated(uint256 indexed shardId, uint256 currentLoad);
    event ShardStateRootUpdated(uint256 indexed shardId, bytes32 stateRoot);
    event EmergencyShardMaintenance(uint256 indexed shardId, string reason);

    // State variables
    mapping(uint256 => ShardInfo) public shards;
    mapping(uint256 => ShardStats) public shardStats;
    mapping(bytes32 => CrossShardRoute) public crossShardRoutes;
    
    RebalanceParams public rebalanceParams;
    EnumerableSet.UintSet private activeShards;
    
    // Constants
    uint256 public constant MAX_SHARDS = 256;
    uint256 public constant MIN_VALIDATORS_PER_SHARD = 4;
    uint256 public constant MAX_LOAD_THRESHOLD = 90; // 90%
    uint256 public constant MIN_LOAD_THRESHOLD = 10; // 10%
    uint256 public constant REBALANCE_COOLDOWN = 1 hours;

    // Modifiers
    modifier validShardId(uint256 shardId) {
        require(shardId < MAX_SHARDS, "ShardManager: Invalid shard ID");
        require(shards[shardId].status != ShardStatus.Inactive, 
                "ShardManager: Shard inactive");
        _;
    }

    modifier onlyShardValidator(uint256 shardId) {
        bool isValidator = false;
        for (uint i = 0; i < shards[shardId].validators.length; i++) {
            if (shards[shardId].validators[i] == msg.sender) {
                isValidator = true;
                break;
            }
        }
        require(isValidator, "ShardManager: Not a shard validator");
        _;
    }

    constructor() {
        _initializeRebalanceParams();
    }

    /**
     * @dev Creates a new shard with specified parameters
     * @param capacity Maximum capacity of the shard
     * @param validators Initial set of validators
     * @return shardId The ID of the created shard
     */
    function createShard(
        uint256 capacity,
        address[] calldata validators
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        returns (uint256 shardId) 
    {
        require(activeShards.length() < MAX_SHARDS, "ShardManager: Max shards reached");
        require(validators.length >= MIN_VALIDATORS_PER_SHARD, 
                "ShardManager: Insufficient validators");

        shardId = _getNextShardId();
        ShardInfo storage newShard = shards[shardId];
        newShard.shardId = shardId;
        newShard.capacity = capacity;
        newShard.creationTime = block.timestamp;
        newShard.status = ShardStatus.Initializing;
        newShard.validators = validators;

        activeShards.add(shardId);
        emit ShardCreated(shardId, capacity);
        return shardId;
    }

    /**
     * @dev Updates shard status
     * @param shardId The shard ID
     * @param status New status
     */
    function updateShardStatus(
        uint256 shardId,
        ShardStatus status
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        validShardId(shardId) 
    {
        shards[shardId].status = status;
        emit ShardStatusUpdated(shardId, status);
    }

    /**
     * @dev Assigns new validator to a shard
     * @param shardId The shard ID
     * @param validator Address of the validator
     */
    function assignValidator(
        uint256 shardId,
        address validator
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        validShardId(shardId) 
    {
        ShardInfo storage shard = shards[shardId];
        require(!_isValidatorInShard(shardId, validator), 
                "ShardManager: Validator already assigned");

        shard.validators.push(validator);
        emit ValidatorAssigned(shardId, validator);
    }

    /**
     * @dev Updates shard load metrics
     * @param shardId The shard ID
     * @param newLoad New load value
     */
    function updateShardLoad(
        uint256 shardId,
        uint256 newLoad
    ) 
        external 
        onlyShardValidator(shardId) 
        validShardId(shardId) 
    {
        require(newLoad <= shards[shardId].capacity, 
                "ShardManager: Load exceeds capacity");

        shards[shardId].currentLoad = newLoad;
        emit ShardLoadUpdated(shardId, newLoad);

        if (_shouldRebalance(shardId)) {
            _triggerRebalancing(shardId);
        }
    }

    /**
     * @dev Updates shard state root
     * @param shardId The shard ID
     * @param newStateRoot New state root
     */
    function updateShardStateRoot(
        uint256 shardId,
        bytes32 newStateRoot
    ) 
        external 
        onlyShardValidator(shardId) 
        validShardId(shardId) 
    {
        shards[shardId].stateRoot = newStateRoot;
        emit ShardStateRootUpdated(shardId, newStateRoot);
    }

    /**
     * @dev Establishes cross-shard route
     * @param sourceShardId Source shard ID
     * @param targetShardId Target shard ID
     * @param latency Expected latency
     * @param bandwidth Available bandwidth
     */
    function establishCrossShardRoute(
        uint256 sourceShardId,
        uint256 targetShardId,
        uint256 latency,
        uint256 bandwidth
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(sourceShardId != targetShardId, 
                "ShardManager: Invalid route");
        
        bytes32 routeId = _getCrossShardRouteId(sourceShardId, targetShardId);
        crossShardRoutes[routeId] = CrossShardRoute({
            sourceShardId: sourceShardId,
            targetShardId: targetShardId,
            latency: latency,
            bandwidth: bandwidth,
            isActive: true
        });

        emit CrossShardRouteEstablished(sourceShardId, targetShardId);
    }

    /**
     * @dev Updates shard statistics
     * @param shardId The shard ID
     * @param stats New statistics
     */
    function updateShardStats(
        uint256 shardId,
        ShardStats calldata stats
    ) 
        external 
        onlyShardValidator(shardId) 
        validShardId(shardId) 
    {
        require(stats.lastUpdateTimestamp > shardStats[shardId].lastUpdateTimestamp, 
                "ShardManager: Stale stats");

        shardStats[shardId] = stats;
    }

    /**
     * @dev Initiates emergency maintenance for a shard
     * @param shardId The shard ID
     * @param reason Reason for maintenance
     */
    function initiateEmergencyMaintenance(
        uint256 shardId,
        string calldata reason
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        validShardId(shardId) 
    {
        shards[shardId].status = ShardStatus.Maintenance;
        emit EmergencyShardMaintenance(shardId, reason);
    }

    // Internal functions

    function _initializeRebalanceParams() internal {
        rebalanceParams = RebalanceParams({
            threshold: 75, // 75%
            cooldownPeriod: REBALANCE_COOLDOWN,
            maxNodesPerMove: 5,
            autoRebalance: true
        });
    }

    function _getNextShardId() internal view returns (uint256) {
        return activeShards.length();
    }

    function _isValidatorInShard(uint256 shardId, address validator) 
        internal 
        view 
        returns (bool) 
    {
        for (uint i = 0; i < shards[shardId].validators.length; i++) {
            if (shards[shardId].validators[i] == validator) {
                return true;
            }
        }
        return false;
    }

    function _shouldRebalance(uint256 shardId) internal view returns (bool) {
        if (!rebalanceParams.autoRebalance) return false;
        
        ShardInfo storage shard = shards[shardId];
        uint256 loadPercentage = shard.currentLoad.mul(100).div(shard.capacity);
        
        return loadPercentage > rebalanceParams.threshold &&
               block.timestamp >= shard.lastRebalanceTime.add(rebalanceParams.cooldownPeriod);
    }

    function _triggerRebalancing(uint256 sourceShardId) internal {
        uint256 targetShardId = _findOptimalTargetShard(sourceShardId);
        if (targetShardId != type(uint256).max) {
            _executeRebalancing(sourceShardId, targetShardId);
        }
    }

    function _findOptimalTargetShard(uint256 sourceShardId) 
        internal 
        view 
        returns (uint256) 
    {
        // Implementation of target shard selection algorithm
        return type(uint256).max;
    }

    function _executeRebalancing(uint256 sourceShardId, uint256 targetShardId) internal {
        shards[sourceShardId].status = ShardStatus.Rebalancing;
        shards[sourceShardId].lastRebalanceTime = block.timestamp;
        
        // Implementation of rebalancing logic
        
        emit ShardRebalanced(sourceShardId, targetShardId);
    }

    function _getCrossShardRouteId(uint256 sourceShardId, uint256 targetShardId) 
        internal 
        pure 
        returns (bytes32) 
    {
        return keccak256(abi.encodePacked(sourceShardId, targetShardId));
    }

    // View functions

    function getShardInfo(uint256 shardId) 
        external 
        view 
        returns (
            uint256 capacity,
            uint256 currentLoad,
            uint256 creationTime,
            uint256 lastRebalanceTime,
            bytes32 stateRoot,
            ShardStatus status,
            address[] memory validators
        ) 
    {
        ShardInfo storage shard = shards[shardId];
        return (
            shard.capacity,
            shard.currentLoad,
            shard.creationTime,
            shard.lastRebalanceTime,
            shard.stateRoot,
            shard.status,
            shard.validators
        );
    }

    function getActiveShards() external view returns (uint256[] memory) {
        return activeShards.values();
    }

    function getShardStats(uint256 shardId) 
        external 
        view 
        returns (ShardStats memory) 
    {
        return shardStats[shardId];
    }

    function getCrossShardRoute(uint256 sourceShardId, uint256 targetShardId) 
        external 
        view 
        returns (CrossShardRoute memory) 
    {
        return crossShardRoutes[_getCrossShardRouteId(sourceShardId, targetShardId)];
    }
}