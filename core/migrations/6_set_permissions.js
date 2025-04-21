const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");
const ShardRegistry = artifacts.require("ShardRegistry");
const ShardRouter = artifacts.require("ShardRouter");
const ConsensusManager = artifacts.require("ConsensusManager");
const ValidatorSet = artifacts.require("ValidatorSet");
const ShardGovernance = artifacts.require("ShardGovernance");

module.exports = async function (deployer, network, accounts) {
  const [deployer, admin, ...others] = accounts;

  // Get all deployed contracts
  const networkParams = await NetworkParams.deployed();
  const shardManager = await ShardManager.deployed();
  const shardRegistry = await ShardRegistry.deployed();
  const shardRouter = await ShardRouter.deployed();
  const consensusManager = await ConsensusManager.deployed();
  const validatorSet = await ValidatorSet.deployed();
  const shardGovernance = await ShardGovernance.deployed();

  // Set up roles and permissions
  const SHARD_ADMIN_ROLE = await networkParams.SHARD_ADMIN_ROLE();
  const VALIDATOR_ROLE = await networkParams.VALIDATOR_ROLE();

  // Grant roles to contracts
  await networkParams.grantRole(SHARD_ADMIN_ROLE, shardManager.address);
  await networkParams.grantRole(SHARD_ADMIN_ROLE, consensusManager.address);
  await networkParams.grantRole(SHARD_ADMIN_ROLE, shardGovernance.address);
  await networkParams.grantRole(VALIDATOR_ROLE, validatorSet.address);

  // Set up cross-contract permissions
  await shardManager.grantRole(SHARD_ADMIN_ROLE, consensusManager.address);
  await shardManager.grantRole(SHARD_ADMIN_ROLE, shardGovernance.address);
  await validatorSet.grantRole(VALIDATOR_ROLE, consensusManager.address);

  // Transfer ownership to admin
  await networkParams.transferOwnership(admin);
  await shardManager.transferOwnership(admin);
  await shardGovernance.transferOwnership(admin);

  console.log("Permissions set up completed");
  console.log("Admin address:", admin);
};