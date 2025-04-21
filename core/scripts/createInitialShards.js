const ShardManager = artifacts.require("ShardManager");
const ShardRegistry = artifacts.require("ShardRegistry");

async function main() {
    try {
        const shardManager = await ShardManager.deployed();
        const shardRegistry = await ShardRegistry.deployed();

        console.log("Creating initial shards...");

        // Initial shard configurations
        const initialShards = [
            {
                capacity: web3.utils.toWei('1000000', 'ether'),
                type: 0, // Standard shard
                name: "Shard-0",
                description: "Genesis shard"
            },
            {
                capacity: web3.utils.toWei('1000000', 'ether'),
                type: 0,
                name: "Shard-1",
                description: "First auxiliary shard"
            }
        ];

        for (const shard of initialShards) {
            // Create shard
            const shardId = await shardManager.createShard(
                shard.capacity,
                [] // Initial validators will be assigned later
            );

            // Register shard
            await shardRegistry.registerShard(
                shardId,
                shard.type,
                web3.utils.utf8ToHex(shard.name),
                web3.utils.utf8ToHex(shard.description)
            );

            console.log(`Shard created and registered: ${shard.name}`);
        }

        console.log("Initial shards creation completed");
    } catch (error) {
        console.error("Shard creation failed:", error);
        process.exit(1);
    }
}

module.exports = main;