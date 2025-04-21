const NetworkParams = artifacts.require("NetworkParams");
const ShardManager = artifacts.require("ShardManager");
const ShardRouter = artifacts.require("ShardRouter");

const EMERGENCY_ACTIONS = {
    PAUSE_NETWORK: 'PAUSE_NETWORK',
    FREEZE_SHARD: 'FREEZE_SHARD',
    SHUTDOWN_ROUTE: 'SHUTDOWN_ROUTE',
    RESTORE_NETWORK: 'RESTORE_NETWORK'
};

async function main(action, params) {
    try {
        const networkParams = await NetworkParams.deployed();
        const shardManager = await ShardManager.deployed();
        const shardRouter = await ShardRouter.deployed();

        console.log("Executing emergency action:", action);

        switch (action) {
            case EMERGENCY_ACTIONS.PAUSE_NETWORK:
                await networkParams.pause();
                console.log("Network paused");
                break;

            case EMERGENCY_ACTIONS.FREEZE_SHARD:
                const { shardId, reason } = params;
                await shardManager.initiateEmergencyMaintenance(shardId, reason);
                console.log(`Shard ${shardId} frozen`);
                break;

            case EMERGENCY_ACTIONS.SHUTDOWN_ROUTE:
                const { sourceShardId, targetShardId } = params;
                await shardRouter.updateRouteStatus(
                    sourceShardId,
                    targetShardId,
                    3 // Failed status
                );
                console.log(`Route ${sourceShardId}->${targetShardId} shut down`);
                break;

            case EMERGENCY_ACTIONS.RESTORE_NETWORK:
                await networkParams.unpause();
                console.log("Network restored");
                break;

            default:
                throw new Error("Unknown emergency action");
        }

        console.log("Emergency action completed successfully");
    } catch (error) {
        console.error("Emergency action failed:", error);
        process.exit(1);
    }
}

module.exports = {
    main,
    EMERGENCY_ACTIONS
};