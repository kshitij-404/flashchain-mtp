const ConsensusManager = artifacts.require("ConsensusManager");
const ValidatorSet = artifacts.require("ValidatorSet");
const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");

module.exports = async function (deployer, network, accounts) {
  const networkParams = await NetworkParams.deployed();
  const shardManager = await ShardManager.deployed();

  // Deploy consensus contracts
  await deployer.deploy(ConsensusManager);
  await deployer.deploy(ValidatorSet, process.env.STAKING_TOKEN_ADDRESS);

  // Get deployed instances
  const consensusManager = await ConsensusManager.deployed();
  const validatorSet = await ValidatorSet.deployed();

  // Initialize contracts with dependencies
  await consensusManager.initialize(
    networkParams.address,
    shardManager.address,
    validatorSet.address
  );
  await validatorSet.initialize(
    networkParams.address,
    consensusManager.address
  );

  console.log("Consensus contracts deployed:");
  console.log("ConsensusManager:", ConsensusManager.address);
  console.log("ValidatorSet:", ValidatorSet.address);
};