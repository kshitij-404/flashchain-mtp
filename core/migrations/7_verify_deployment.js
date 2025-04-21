const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");
const ShardRegistry = artifacts.require("ShardRegistry");
const ShardRouter = artifacts.require("ShardRouter");
const ConsensusManager = artifacts.require("ConsensusManager");
const ValidatorSet = artifacts.require("ValidatorSet");
const ShardGovernance = artifacts.require("ShardGovernance");

module.exports = async function (deployer, network, accounts) {
  // Verify all contracts are deployed and initialized
  const contracts = {
    NetworkParams: await NetworkParams.deployed(),
    ShardManager: await ShardManager.deployed(),
    ShardRegistry: await ShardRegistry.deployed(),
    ShardRouter: await ShardRouter.deployed(),
    ConsensusManager: await ConsensusManager.deployed(),
    ValidatorSet: await ValidatorSet.deployed(),
    ShardGovernance: await ShardGovernance.deployed()
  };

  console.log("\nDeployment Verification Report:");
  console.log("================================");

  for (const [name, contract] of Object.entries(contracts)) {
    console.log(`\n${name}:`);
    console.log("Address:", contract.address);
    console.log("Has code:", await web3.eth.getCode(contract.address) !== '0x');
    
    // Verify initialization
    try {
      const initialized = await contract.initialized();
      console.log("Initialized:", initialized);
    } catch (error) {
      console.log("Initialization check failed");
    }
  }

  // Verify key permissions
  const networkParams = contracts.NetworkParams;
  const SHARD_ADMIN_ROLE = await networkParams.SHARD_ADMIN_ROLE();
  const VALIDATOR_ROLE = await networkParams.VALIDATOR_ROLE();

  console.log("\nPermissions Verification:");
  console.log("========================");
  console.log("ShardManager has SHARD_ADMIN_ROLE:", 
    await networkParams.hasRole(SHARD_ADMIN_ROLE, contracts.ShardManager.address));
  console.log("ConsensusManager has SHARD_ADMIN_ROLE:", 
    await networkParams.hasRole(SHARD_ADMIN_ROLE, contracts.ConsensusManager.address));
  console.log("ValidatorSet has VALIDATOR_ROLE:", 
    await networkParams.hasRole(VALIDATOR_ROLE, contracts.ValidatorSet.address));

  console.log("\nDeployment verification completed");
};