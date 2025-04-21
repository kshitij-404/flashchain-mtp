const ShardGovernance = artifacts.require("ShardGovernance");
const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");
const ValidatorSet = artifacts.require("ValidatorSet");

module.exports = async function (deployer, network, accounts) {
  const networkParams = await NetworkParams.deployed();
  const shardManager = await ShardManager.deployed();
  const validatorSet = await ValidatorSet.deployed();

  // Deploy governance contracts
  await deployer.deploy(ShardGovernance);

  // Get deployed instance
  const shardGovernance = await ShardGovernance.deployed();

  // Initialize contracts with dependencies
  await shardGovernance.initialize(
    networkParams.address,
    shardManager.address,
    validatorSet.address
  );

  console.log("Governance contracts deployed:");
  console.log("ShardGovernance:", ShardGovernance.address);
};