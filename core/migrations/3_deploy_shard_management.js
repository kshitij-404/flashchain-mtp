const ShardManager = artifacts.require("ShardManager");
const ShardRegistry = artifacts.require("ShardRegistry");
const ShardRouter = artifacts.require("ShardRouter");
const NetworkParams = artifacts.require("NetworkParams");

module.exports = async function (deployer, network, accounts) {
  const networkParams = await NetworkParams.deployed();

  // Deploy shard management contracts
  await deployer.deploy(ShardManager);
  await deployer.deploy(ShardRegistry);
  await deployer.deploy(ShardRouter);

  // Get deployed instances
  const shardManager = await ShardManager.deployed();
  const shardRegistry = await ShardRegistry.deployed();
  const shardRouter = await ShardRouter.deployed();

  // Initialize contracts with dependencies
  await shardManager.initialize(networkParams.address);
  await shardRegistry.initialize(networkParams.address, shardManager.address);
  await shardRouter.initialize(networkParams.address, shardManager.address);

  console.log("Shard management contracts deployed:");
  console.log("ShardManager:", ShardManager.address);
  console.log("ShardRegistry:", ShardRegistry.address);
  console.log("ShardRouter:", ShardRouter.address);
};