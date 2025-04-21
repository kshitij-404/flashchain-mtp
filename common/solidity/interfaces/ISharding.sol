// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface ISharding {
    // Events
    event ShardCreated(uint256 indexed shardId, address[] validators);
    event ShardUpdated(uint256 indexed shardId, bytes32 stateHash);
    event ValidatorAssigned(uint256 indexed shardId, address indexed validator);
    event ValidatorUnassigned(uint256 indexed shardId, address indexed validator);

    // Structs
    struct ShardInfo {
        uint256 shardId;
        address[] validators;
        uint256 capacity;
        bytes32 stateRoot;
        bool isActive;
        uint256 lastUpdateTime;
    }

    // Core functions
    function createShard(address[] calldata validators) external returns (uint256);
    function assignValidatorToShard(uint256 shardId, address validator) external;
    function removeValidatorFromShard(uint256 shardId, address validator) external;
    function updateShardState(uint256 shardId, bytes32 stateHash) external;
    function getShardInfo(uint256 shardId) external view returns (ShardInfo memory);
    function getValidatorShards(address validator) external view returns (uint256[] memory);
}