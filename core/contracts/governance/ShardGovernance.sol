// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../base/BaseShardContract.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

/**
 * @title ShardGovernance 
 * @dev Manages governance mechanisms for shards in the FlashChain network
 */
contract ShardGovernance is BaseShardContract, Pausable, ReentrancyGuard {
    using SafeMath for uint256;
    using EnumerableSet for EnumerableSet.AddressSet;
    using EnumerableSet for EnumerableSet.UintSet;

    // Structs
    struct Proposal {
        uint256 id;
        uint256 shardId;
        address proposer;
        ProposalType proposalType;
        bytes32 contentHash;
        uint256 startTime;
        uint256 endTime;
        uint256 forVotes;
        uint256 againstVotes;
        ProposalStatus status;
        mapping(address => Vote) votes;
        mapping(bytes32 => bool) executionHashes;
    }

    struct Vote {
        bool hasVoted;
        bool support;
        uint256 votingPower;
        uint256 timestamp;
    }

    struct ShardGovernanceConfig {
        uint256 proposalThreshold;
        uint256 votingPeriod;
        uint256 votingDelay;
        uint256 executionDelay;
        uint256 quorumPercentage;
        uint256 superMajorityPercentage;
    }

    struct ProposalExecutionParams {
        uint256 proposalId;
        bytes[] callData;
        address[] targets;
        uint256[] values;
        string[] descriptions;
    }

    // Enums
    enum ProposalType {
        ShardConfig,
        ValidatorManagement,
        ProtocolUpgrade,
        EmergencyAction,
        ResourceAllocation,
        CrossShardPolicy
    }

    enum ProposalStatus {
        Pending,
        Active,
        Canceled,
        Defeated,
        Succeeded,
        Queued,
        Executed,
        Expired
    }

    // Events
    event ProposalCreated(
        uint256 indexed proposalId,
        uint256 indexed shardId,
        address indexed proposer,
        ProposalType proposalType
    );
    event ProposalCanceled(uint256 indexed proposalId);
    event ProposalQueued(uint256 indexed proposalId, uint256 executionTime);
    event ProposalExecuted(uint256 indexed proposalId);
    event VoteCast(
        address indexed voter,
        uint256 indexed proposalId,
        bool support,
        uint256 votingPower
    );
    event GovernanceConfigUpdated(uint256 indexed shardId);
    event EmergencyActionExecuted(
        uint256 indexed shardId,
        bytes32 indexed actionHash,
        string reason
    );
    event ProposalThresholdUpdated(uint256 indexed shardId, uint256 newThreshold);
    event QuorumUpdated(uint256 indexed shardId, uint256 newQuorum);

    // State variables
    mapping(uint256 => Proposal) public proposals;
    mapping(uint256 => ShardGovernanceConfig) public shardConfigs;
    mapping(uint256 => EnumerableSet.AddressSet) private shardGovernors;
    mapping(uint256 => EnumerableSet.UintSet) private activeProposals;
    
    Counters.Counter private _proposalIdCounter;

    // Constants
    uint256 public constant MAX_PROPOSAL_VALIDITY = 30 days;
    uint256 public constant MIN_VOTING_PERIOD = 1 days;
    uint256 public constant MAX_VOTING_PERIOD = 14 days;
    uint256 public constant MIN_VOTING_DELAY = 1 hours;
    uint256 public constant MAX_EXECUTION_DELAY = 3 days;
    uint256 public constant MIN_QUORUM_PERCENTAGE = 10; // 10%
    uint256 public constant MAX_ACTIONS_PER_PROPOSAL = 10;

    // Modifiers
    modifier onlyShardGovernor(uint256 shardId) {
        require(shardGovernors[shardId].contains(msg.sender), 
                "ShardGovernance: Not a shard governor");
        _;
    }

    modifier validProposal(uint256 proposalId) {
        require(proposals[proposalId].startTime != 0, "ShardGovernance: Invalid proposal");
        _;
    }

    constructor() {
        _initializeDefaultConfigs();
    }

    /**
     * @dev Creates a new governance proposal
     * @param shardId The shard ID for which the proposal is created
     * @param proposalType Type of the proposal
     * @param description Description of the proposal
     * @param executionParams Parameters for proposal execution
     */
    function createProposal(
        uint256 shardId,
        ProposalType proposalType,
        string calldata description,
        ProposalExecutionParams calldata executionParams
    ) 
        external 
        nonReentrant 
        onlyShardGovernor(shardId) 
        returns (uint256) 
    {
        require(
            executionParams.targets.length > 0 && 
            executionParams.targets.length <= MAX_ACTIONS_PER_PROPOSAL,
            "ShardGovernance: Invalid action count"
        );
        require(
            executionParams.targets.length == executionParams.values.length &&
            executionParams.targets.length == executionParams.callData.length,
            "ShardGovernance: Parameter length mismatch"
        );

        ShardGovernanceConfig storage config = shardConfigs[shardId];
        uint256 proposalId = _proposalIdCounter.current();
        _proposalIdCounter.increment();

        Proposal storage proposal = proposals[proposalId];
        proposal.id = proposalId;
        proposal.shardId = shardId;
        proposal.proposer = msg.sender;
        proposal.proposalType = proposalType;
        proposal.contentHash = keccak256(abi.encode(description, executionParams));
        proposal.startTime = block.timestamp.add(config.votingDelay);
        proposal.endTime = proposal.startTime.add(config.votingPeriod);
        proposal.status = ProposalStatus.Pending;

        activeProposals[shardId].add(proposalId);

        emit ProposalCreated(proposalId, shardId, msg.sender, proposalType);
        return proposalId;
    }

    /**
     * @dev Casts a vote on a proposal
     * @param proposalId The ID of the proposal
     * @param support Boolean indicating support for the proposal
     */
    function castVote(
        uint256 proposalId, 
        bool support
    ) 
        external 
        validProposal(proposalId) 
        nonReentrant 
    {
        Proposal storage proposal = proposals[proposalId];
        require(
            block.timestamp >= proposal.startTime && 
            block.timestamp <= proposal.endTime,
            "ShardGovernance: Voting closed"
        );
        require(
            !proposal.votes[msg.sender].hasVoted,
            "ShardGovernance: Already voted"
        );

        uint256 votingPower = _calculateVotingPower(msg.sender, proposal.shardId);
        require(votingPower > 0, "ShardGovernance: No voting power");

        if (support) {
            proposal.forVotes = proposal.forVotes.add(votingPower);
        } else {
            proposal.againstVotes = proposal.againstVotes.add(votingPower);
        }

        proposal.votes[msg.sender] = Vote({
            hasVoted: true,
            support: support,
            votingPower: votingPower,
            timestamp: block.timestamp
        });

        emit VoteCast(msg.sender, proposalId, support, votingPower);
    }

    /**
     * @dev Queues a successful proposal for execution
     * @param proposalId The ID of the proposal to queue
     */
    function queueProposal(uint256 proposalId) 
        external 
        validProposal(proposalId) 
        nonReentrant 
    {
        Proposal storage proposal = proposals[proposalId];
        require(
            _isProposalSucceeded(proposalId),
            "ShardGovernance: Proposal not successful"
        );
        require(
            proposal.status == ProposalStatus.Succeeded,
            "ShardGovernance: Invalid proposal status"
        );

        proposal.status = ProposalStatus.Queued;
        emit ProposalQueued(proposalId, block.timestamp);
    }

    /**
     * @dev Executes a queued proposal
     * @param proposalId The ID of the proposal to execute
     * @param executionParams Parameters for executing the proposal
     */
    function executeProposal(
        uint256 proposalId,
        ProposalExecutionParams calldata executionParams
    ) 
        external 
        validProposal(proposalId) 
        nonReentrant 
    {
        Proposal storage proposal = proposals[proposalId];
        require(
            proposal.status == ProposalStatus.Queued,
            "ShardGovernance: Proposal not queued"
        );
        require(
            block.timestamp >= proposal.endTime.add(
                shardConfigs[proposal.shardId].executionDelay
            ),
            "ShardGovernance: Execution delay not met"
        );

        _executeActions(proposal.shardId, executionParams);
        proposal.status = ProposalStatus.Executed;
        
        activeProposals[proposal.shardId].remove(proposalId);
        emit ProposalExecuted(proposalId);
    }

    /**
     * @dev Updates governance configuration for a shard
     * @param shardId The shard ID
     * @param newConfig The new governance configuration
     */
    function updateShardGovernanceConfig(
        uint256 shardId,
        ShardGovernanceConfig calldata newConfig
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(
            newConfig.votingPeriod >= MIN_VOTING_PERIOD &&
            newConfig.votingPeriod <= MAX_VOTING_PERIOD,
            "ShardGovernance: Invalid voting period"
        );
        require(
            newConfig.votingDelay >= MIN_VOTING_DELAY,
            "ShardGovernance: Invalid voting delay"
        );
        require(
            newConfig.quorumPercentage >= MIN_QUORUM_PERCENTAGE,
            "ShardGovernance: Invalid quorum percentage"
        );

        shardConfigs[shardId] = newConfig;
        emit GovernanceConfigUpdated(shardId);
    }

    // Internal functions

    function _initializeDefaultConfigs() internal {
        ShardGovernanceConfig memory defaultConfig = ShardGovernanceConfig({
            proposalThreshold: 100 ether,
            votingPeriod: 3 days,
            votingDelay: 6 hours,
            executionDelay: 24 hours,
            quorumPercentage: 20, // 20%
            superMajorityPercentage: 67 // 67%
        });

        // Initialize config for main shard
        shardConfigs[0] = defaultConfig;
    }

    function _calculateVotingPower(address voter, uint256 shardId) 
        internal 
        view 
        returns (uint256) 
    {
        // Implement voting power calculation based on stake, reputation, etc.
        return 0;
    }

    function _isProposalSucceeded(uint256 proposalId) 
        internal 
        view 
        returns (bool) 
    {
        Proposal storage proposal = proposals[proposalId];
        ShardGovernanceConfig storage config = shardConfigs[proposal.shardId];

        uint256 totalVotes = proposal.forVotes.add(proposal.againstVotes);
        if (totalVotes < config.quorumPercentage) {
            return false;
        }

        return proposal.forVotes.mul(100) >= totalVotes.mul(config.superMajorityPercentage);
    }

    function _executeActions(
        uint256 shardId,
        ProposalExecutionParams memory params
    ) 
        internal 
    {
        for (uint256 i = 0; i < params.targets.length; i++) {
            (bool success, ) = params.targets[i].call{value: params.values[i]}(
                params.callData[i]
            );
            require(success, "ShardGovernance: Action execution failed");
        }
    }

    // View functions

    function getProposal(uint256 proposalId)
        external
        view
        returns (
            uint256 id,
            uint256 shardId,
            address proposer,
            ProposalType proposalType,
            bytes32 contentHash,
            uint256 startTime,
            uint256 endTime,
            uint256 forVotes,
            uint256 againstVotes,
            ProposalStatus status
        )
    {
        Proposal storage proposal = proposals[proposalId];
        return (
            proposal.id,
            proposal.shardId,
            proposal.proposer,
            proposal.proposalType,
            proposal.contentHash,
            proposal.startTime,
            proposal.endTime,
            proposal.forVotes,
            proposal.againstVotes,
            proposal.status
        );
    }

    function getVoteDetails(uint256 proposalId, address voter)
        external
        view
        returns (Vote memory)
    {
        return proposals[proposalId].votes[voter];
    }

    function getActiveProposals(uint256 shardId)
        external
        view
        returns (uint256[] memory)
    {
        return activeProposals[shardId].values();
    }

    function isShardGovernor(uint256 shardId, address account)
        external
        view
        returns (bool)
    {
        return shardGovernors[shardId].contains(account);
    }
}