// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../base/BaseShardContract.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "@openzeppelin/contracts/utils/structs/EnumerableSet.sol";

/**
 * @title ValidatorSet
 * @dev Manages the set of validators across shards in the FlashChain network
 */
contract ValidatorSet is BaseShardContract, ReentrancyGuard {
    using SafeMath for uint256;
    using EnumerableSet for EnumerableSet.AddressSet;

    // Structs
    struct ValidatorDetails {
        address payable validatorAddress;
        uint256 stake;
        uint256 totalRewards;
        uint256 commissionRate;
        uint256 joinTimestamp;
        uint256 lastActiveTimestamp;
        ValidatorStatus status;
        bytes32 publicKey;
        string metadata;
        ShardAssignment[] assignedShards;
    }

    struct ShardAssignment {
        uint256 shardId;
        uint256 assignmentTimestamp;
        uint256 performanceScore;
        bool isActive;
    }

    struct StakingPool {
        uint256 totalStaked;
        uint256 rewardRate;
        uint256 lastUpdateTime;
        mapping(address => uint256) delegatorStakes;
        EnumerableSet.AddressSet delegators;
    }

    enum ValidatorStatus {
        Inactive,
        Pending,
        Active,
        Jailed,
        Slashed
    }

    // Events
    event ValidatorRegistered(address indexed validator, uint256 stake, bytes32 publicKey);
    event ValidatorActivated(address indexed validator);
    event ValidatorDeactivated(address indexed validator);
    event ValidatorSlashed(address indexed validator, uint256 amount, string reason);
    event ValidatorJailed(address indexed validator, uint256 duration);
    event StakeAdded(address indexed validator, uint256 amount);
    event StakeWithdrawn(address indexed validator, uint256 amount);
    event RewardClaimed(address indexed validator, uint256 amount);
    event DelegationAdded(address indexed validator, address indexed delegator, uint256 amount);
    event DelegationWithdrawn(address indexed validator, address indexed delegator, uint256 amount);
    event ShardAssigned(address indexed validator, uint256 indexed shardId);
    event PerformanceScoreUpdated(address indexed validator, uint256 indexed shardId, uint256 score);

    // State variables
    mapping(address => ValidatorDetails) public validators;
    mapping(address => StakingPool) private stakingPools;
    mapping(uint256 => EnumerableSet.AddressSet) private shardValidators;
    
    IERC20 public immutable stakingToken;
    
    // Configuration
    uint256 public constant MIN_STAKE = 100000 ether;
    uint256 public constant MAX_COMMISSION_RATE = 2000; // 20% in basis points
    uint256 public constant SLASH_MULTIPLIER = 200; // 2x multiplier
    uint256 public constant JAIL_DURATION = 7 days;
    uint256 public constant PERFORMANCE_UPDATE_INTERVAL = 1 hours;
    uint256 public constant MAX_VALIDATORS_PER_SHARD = 100;

    constructor(address _stakingToken) {
        require(_stakingToken != address(0), "ValidatorSet: Invalid staking token");
        stakingToken = IERC20(_stakingToken);
    }

    /**
     * @dev Registers a new validator
     * @param publicKey Validator's public key for consensus participation
     * @param commissionRate Validator's commission rate in basis points
     * @param metadata Additional validator metadata (IPFS hash, etc.)
     */
    function registerValidator(
        bytes32 publicKey,
        uint256 commissionRate,
        string calldata metadata
    ) 
        external 
        nonReentrant 
    {
        require(validators[msg.sender].status == ValidatorStatus.Inactive, 
                "ValidatorSet: Validator already registered");
        require(commissionRate <= MAX_COMMISSION_RATE, 
                "ValidatorSet: Commission rate too high");

        uint256 stake = MIN_STAKE;
        require(stakingToken.transferFrom(msg.sender, address(this), stake),
                "ValidatorSet: Stake transfer failed");

        validators[msg.sender] = ValidatorDetails({
            validatorAddress: payable(msg.sender),
            stake: stake,
            totalRewards: 0,
            commissionRate: commissionRate,
            joinTimestamp: block.timestamp,
            lastActiveTimestamp: block.timestamp,
            status: ValidatorStatus.Pending,
            publicKey: publicKey,
            metadata: metadata,
            assignedShards: new ShardAssignment[](0)
        });

        emit ValidatorRegistered(msg.sender, stake, publicKey);
    }

    /**
     * @dev Activates a validator for consensus participation
     * @param validator Address of the validator to activate
     */
    function activateValidator(address validator) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(validators[validator].status == ValidatorStatus.Pending, 
                "ValidatorSet: Invalid validator status");
        
        validators[validator].status = ValidatorStatus.Active;
        validators[validator].lastActiveTimestamp = block.timestamp;

        emit ValidatorActivated(validator);
    }

    /**
     * @dev Assigns a validator to a shard
     * @param validator Address of the validator
     * @param shardId ID of the shard to assign
     */
    function assignValidatorToShard(
        address validator, 
        uint256 shardId
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(validators[validator].status == ValidatorStatus.Active, 
                "ValidatorSet: Validator not active");
        require(shardValidators[shardId].length() < MAX_VALIDATORS_PER_SHARD, 
                "ValidatorSet: Shard validator limit reached");

        ShardAssignment memory assignment = ShardAssignment({
            shardId: shardId,
            assignmentTimestamp: block.timestamp,
            performanceScore: 100,
            isActive: true
        });

        validators[validator].assignedShards.push(assignment);
        shardValidators[shardId].add(validator);

        emit ShardAssigned(validator, shardId);
    }

    /**
     * @dev Updates validator's performance score
     * @param validator Address of the validator
     * @param shardId ID of the shard
     * @param score New performance score
     */
    function updatePerformanceScore(
        address validator,
        uint256 shardId,
        uint256 score
    ) 
        external 
        onlyRole(VALIDATOR_ROLE) 
    {
        require(score <= 100, "ValidatorSet: Invalid score");
        
        for (uint i = 0; i < validators[validator].assignedShards.length; i++) {
            if (validators[validator].assignedShards[i].shardId == shardId) {
                validators[validator].assignedShards[i].performanceScore = score;
                emit PerformanceScoreUpdated(validator, shardId, score);
                break;
            }
        }
    }

    /**
     * @dev Slashes a validator's stake for misbehavior
     * @param validator Address of the validator to slash
     * @param reason Reason for slashing
     */
    function slashValidator(
        address validator,
        string calldata reason
    ) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(validators[validator].status == ValidatorStatus.Active, 
                "ValidatorSet: Validator not active");

        uint256 slashAmount = validators[validator].stake.mul(SLASH_MULTIPLIER).div(100);
        validators[validator].stake = validators[validator].stake.sub(slashAmount);
        validators[validator].status = ValidatorStatus.Slashed;

        emit ValidatorSlashed(validator, slashAmount, reason);
    }

    /**
     * @dev Jails a validator for poor performance
     * @param validator Address of the validator to jail
     */
    function jailValidator(address validator) 
        external 
        onlyRole(SHARD_ADMIN_ROLE) 
    {
        require(validators[validator].status == ValidatorStatus.Active, 
                "ValidatorSet: Validator not active");

        validators[validator].status = ValidatorStatus.Jailed;
        validators[validator].lastActiveTimestamp = block.timestamp.add(JAIL_DURATION);

        emit ValidatorJailed(validator, JAIL_DURATION);
    }

    /**
     * @dev Allows delegators to stake tokens to a validator's pool
     * @param validator Address of the validator
     * @param amount Amount to delegate
     */
    function delegateStake(
        address validator,
        uint256 amount
    ) 
        external 
        nonReentrant 
    {
        require(validators[validator].status == ValidatorStatus.Active, 
                "ValidatorSet: Validator not active");
        require(amount > 0, "ValidatorSet: Invalid amount");

        StakingPool storage pool = stakingPools[validator];
        require(stakingToken.transferFrom(msg.sender, address(this), amount),
                "ValidatorSet: Transfer failed");

        pool.delegatorStakes[msg.sender] = pool.delegatorStakes[msg.sender].add(amount);
        pool.totalStaked = pool.totalStaked.add(amount);
        pool.delegators.add(msg.sender);

        emit DelegationAdded(validator, msg.sender, amount);
    }

    /**
     * @dev Claims accumulated rewards for a validator
     */
    function claimRewards() 
        external 
        nonReentrant 
    {
        ValidatorDetails storage validator = validators[msg.sender];
        require(validator.status == ValidatorStatus.Active, 
                "ValidatorSet: Validator not active");

        uint256 rewards = _calculateRewards(msg.sender);
        require(rewards > 0, "ValidatorSet: No rewards to claim");

        validator.totalRewards = validator.totalRewards.add(rewards);
        require(stakingToken.transfer(msg.sender, rewards),
                "ValidatorSet: Reward transfer failed");

        emit RewardClaimed(msg.sender, rewards);
    }

    // Internal functions

    function _calculateRewards(address validator) 
        internal 
        view 
        returns (uint256) 
    {
        // Implement reward calculation logic
        return 0;
    }

    // View functions

    function getValidatorDetails(address validator) 
        external 
        view 
        returns (ValidatorDetails memory) 
    {
        return validators[validator];
    }

    function getShardValidators(uint256 shardId) 
        external 
        view 
        returns (address[] memory) 
    {
        return shardValidators[shardId].values();
    }

    function getDelegatorStake(
        address validator,
        address delegator
    ) 
        external 
        view 
        returns (uint256) 
    {
        return stakingPools[validator].delegatorStakes[delegator];
    }

    function isValidValidator(address validator) 
        external 
        view 
        returns (bool) 
    {
        return validators[validator].status == ValidatorStatus.Active;
    }
}