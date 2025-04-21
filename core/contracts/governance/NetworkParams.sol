// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../base/BaseShardContract.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

/**
 * @title NetworkParams
 * @dev Manages global network parameters and configurations for the FlashChain network
 */
contract NetworkParams is BaseShardContract, Pausable, ReentrancyGuard {
    using SafeMath for uint256;

    // Structs
    struct NetworkConfiguration {
        uint256 maxShards;
        uint256 minNodesPerShard;
        uint256 maxNodesPerShard;
        uint256 consensusThreshold;
        uint256 blockInterval;
        uint256 epochDuration;
        uint256 validatorMinStake;
        uint256 delegatorMinStake;
        uint256 crossShardTxGasMultiplier;
        uint256 baseRewardRate;
        uint256 slashingPenalty;
        bool dynamicSharding;
    }

    struct ShardingPolicy {
        uint256 targetShardSize;
        uint256 reshardingThreshold;
        uint256 minShardLoad;
        uint256 maxShardLoad;
        uint256 loadBalanceInterval;
        bool autoResharding;
    }

    struct PerformanceThresholds {
        uint256 minTPS;
        uint256 targetTPS;
        uint256 maxLatency;
        uint256 minValidatorUptime;
        uint256 slashingThreshold;
        uint256 jailThreshold;
    }

    struct NetworkMetrics {
        uint256 totalNodes;
        uint256 activeShards;
        uint256 totalTransactions;
        uint256 averageBlockTime;
        uint256 networkLoad;
        uint256 lastUpdateTimestamp;
    }

    // Events
    event NetworkConfigUpdated(bytes32 indexed paramHash);
    event ShardingPolicyUpdated(bytes32 indexed policyHash);
    event PerformanceThresholdsUpdated(bytes32 indexed thresholdHash);
    event NetworkMetricsUpdated(uint256 timestamp);
    event EmergencyModeActivated(string reason);
    event EmergencyModeDeactivated();
    event ParameterProposed(bytes32 indexed proposalId, address indexed proposer);
    event ParameterApproved(bytes32 indexed proposalId);
    event ParameterRejected(bytes32 indexed proposalId);

    // State variables
    NetworkConfiguration public networkConfig;
    ShardingPolicy public shardingPolicy;
    PerformanceThresholds public performanceThresholds;
    NetworkMetrics public networkMetrics;

    // Parameter update proposal system
    struct ParameterProposal {
        address proposer;
        bytes32 parameterHash;
        uint256 proposalTime;
        uint256 approvalCount;
        bool executed;
        mapping(address => bool) approvals;
    }

    mapping(bytes32 => ParameterProposal) public proposals;
    mapping(bytes32 => mapping(address => uint256)) public parameterHistory;
    
    // Constants
    uint256 public constant PROPOSAL_EXPIRY = 7 days;
    uint256 public constant MIN_APPROVALS = 3;
    uint256 public constant EMERGENCY_TIMEOUT = 24 hours;
    uint256 public constant MAX_GAS_MULTIPLIER = 5;
    uint256 public constant DEFAULT_BLOCK_INTERVAL = 15 seconds;

    // Modifiers
    modifier onlyWithProposal(bytes32 proposalId) {
        require(proposals[proposalId].proposer != address(0), "NetworkParams: Invalid proposal");
        require(!proposals[proposalId].executed, "NetworkParams: Proposal already executed");
        require(block.timestamp <= proposals[proposalId].proposalTime + PROPOSAL_EXPIRY, 
                "NetworkParams: Proposal expired");
        _;
    }

    constructor() {
        _initializeDefaultConfig();
    }

    /**
     * @dev Initializes default network configuration
     */
    function _initializeDefaultConfig() internal {
        networkConfig = NetworkConfiguration({
            maxShards: 32,
            minNodesPerShard: 4,
            maxNodesPerShard: 100,
            consensusThreshold: 67, // 67%
            blockInterval: DEFAULT_BLOCK_INTERVAL,
            epochDuration: 1 hours,
            validatorMinStake: 100000 ether,
            delegatorMinStake: 1000 ether,
            crossShardTxGasMultiplier: 2,
            baseRewardRate: 500, // 5% annual in basis points
            slashingPenalty: 1000, // 10% in basis points
            dynamicSharding: true
        });

        shardingPolicy = ShardingPolicy({
            targetShardSize: 50,
            reshardingThreshold: 80, // 80% load
            minShardLoad: 20, // 20% minimum load
            maxShardLoad: 90, // 90% maximum load
            loadBalanceInterval: 1 hours,
            autoResharding: true
        });

        performanceThresholds = PerformanceThresholds({
            minTPS: 1000,
            targetTPS: 5000,
            maxLatency: 3 seconds,
            minValidatorUptime: 95, // 95%
            slashingThreshold: 3, // 3 strikes
            jailThreshold: 5 // 5 strikes
        });

        networkMetrics = NetworkMetrics({
            totalNodes: 0,
            activeShards: 0,
            totalTransactions: 0,
            averageBlockTime: DEFAULT_BLOCK_INTERVAL,
            networkLoad: 0,
            lastUpdateTimestamp: block.timestamp
        });
    }

    /**
     * @dev Proposes a new network configuration
     * @param newConfig The proposed network configuration
     * @return proposalId The ID of the created proposal
     */
    function proposeNetworkConfig(NetworkConfiguration calldata newConfig) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        returns (bytes32 proposalId) 
    {
        require(newConfig.maxShards > 0, "NetworkParams: Invalid max shards");
        require(newConfig.minNodesPerShard >= 4, "NetworkParams: Invalid min nodes");
        require(newConfig.crossShardTxGasMultiplier <= MAX_GAS_MULTIPLIER, 
                "NetworkParams: Invalid gas multiplier");

        proposalId = keccak256(abi.encode(newConfig, block.timestamp, msg.sender));
        
        ParameterProposal storage proposal = proposals[proposalId];
        proposal.proposer = msg.sender;
        proposal.parameterHash = keccak256(abi.encode(newConfig));
        proposal.proposalTime = block.timestamp;
        proposal.approvals[msg.sender] = true;
        proposal.approvalCount = 1;

        emit ParameterProposed(proposalId, msg.sender);
        return proposalId;
    }

    /**
     * @dev Approves a parameter proposal
     * @param proposalId The ID of the proposal to approve
     */
    function approveProposal(bytes32 proposalId) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
        onlyWithProposal(proposalId) 
    {
        ParameterProposal storage proposal = proposals[proposalId];
        require(!proposal.approvals[msg.sender], "NetworkParams: Already approved");

        proposal.approvals[msg.sender] = true;
        proposal.approvalCount = proposal.approvalCount.add(1);

        if (proposal.approvalCount >= MIN_APPROVALS) {
            _executeProposal(proposalId);
        }
    }

    /**
     * @dev Updates sharding policy
     * @param newPolicy The new sharding policy
     */
    function updateShardingPolicy(ShardingPolicy calldata newPolicy) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(newPolicy.targetShardSize >= networkConfig.minNodesPerShard, 
                "NetworkParams: Invalid target size");
        require(newPolicy.reshardingThreshold <= 100, "NetworkParams: Invalid threshold");

        shardingPolicy = newPolicy;
        emit ShardingPolicyUpdated(keccak256(abi.encode(newPolicy)));
    }

    /**
     * @dev Updates performance thresholds
     * @param newThresholds The new performance thresholds
     */
    function updatePerformanceThresholds(PerformanceThresholds calldata newThresholds) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(newThresholds.minTPS > 0, "NetworkParams: Invalid min TPS");
        require(newThresholds.maxLatency > 0, "NetworkParams: Invalid max latency");

        performanceThresholds = newThresholds;
        emit PerformanceThresholdsUpdated(keccak256(abi.encode(newThresholds)));
    }

    /**
     * @dev Updates network metrics
     * @param metrics The new network metrics
     */
    function updateNetworkMetrics(NetworkMetrics calldata metrics) 
        external 
        onlyRole(VALIDATOR_ROLE) 
    {
        require(metrics.lastUpdateTimestamp > networkMetrics.lastUpdateTimestamp, 
                "NetworkParams: Stale metrics");

        networkMetrics = metrics;
        emit NetworkMetricsUpdated(block.timestamp);
    }

    /**
     * @dev Activates emergency mode
     * @param reason The reason for emergency activation
     */
    function activateEmergencyMode(string calldata reason) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        _pause();
        emit EmergencyModeActivated(reason);
    }

    /**
     * @dev Deactivates emergency mode
     */
    function deactivateEmergencyMode() 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        _unpause();
        emit EmergencyModeDeactivated();
    }

    // Internal functions

    function _executeProposal(bytes32 proposalId) internal {
        ParameterProposal storage proposal = proposals[proposalId];
        proposal.executed = true;
        
        // Implementation of parameter update logic
        // This would decode the proposal and update the relevant parameters
        
        emit ParameterApproved(proposalId);
    }

    // View functions

    function getNetworkConfig() 
        external 
        view 
        returns (NetworkConfiguration memory) 
    {
        return networkConfig;
    }

    function getShardingPolicy() 
        external 
        view 
        returns (ShardingPolicy memory) 
    {
        return shardingPolicy;
    }

    function getPerformanceThresholds() 
        external 
        view 
        returns (PerformanceThresholds memory) 
    {
        return performanceThresholds;
    }

    function getNetworkMetrics() 
        external 
        view 
        returns (NetworkMetrics memory) 
    {
        return networkMetrics;
    }

    function getProposalDetails(bytes32 proposalId) 
        external 
        view 
        returns (
            address proposer,
            bytes32 parameterHash,
            uint256 proposalTime,
            uint256 approvalCount,
            bool executed
        ) 
    {
        ParameterProposal storage proposal = proposals[proposalId];
        return (
            proposal.proposer,
            proposal.parameterHash,
            proposal.proposalTime,
            proposal.approvalCount,
            proposal.executed
        );
    }
}