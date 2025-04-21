const BaseShardContract = artifacts.require("BaseShardContract");
const NetworkParams = artifacts.require("NetworkParams");

module.exports = async function (deployer, network, accounts) {
  // Deploy base contracts
  await deployer.deploy(BaseShardContract);
  await deployer.deploy(NetworkParams);

  console.log("Base contracts deployed:");
  console.log("BaseShardContract:", BaseShardContract.address);
  console.log("NetworkParams:", NetworkParams.address);
};