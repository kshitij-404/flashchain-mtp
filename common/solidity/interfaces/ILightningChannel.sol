// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface ILightningChannel {
    // Events
    event ChannelOpened(bytes32 indexed channelId, address[] participants);
    event ChannelClosed(bytes32 indexed channelId, bytes32 finalState);
    event StateUpdated(bytes32 indexed channelId, bytes32 newState);
    event HTLCCreated(bytes32 indexed channelId, bytes32 indexed htlcId);
    event HTLCResolved(bytes32 indexed channelId, bytes32 indexed htlcId);

    // Structs
    struct ChannelState {
        address[] participants;
        mapping(address => uint256) balances;
        uint256 totalDeposit;
        uint256 numHTLCs;
        bool isOpen;
        uint256 disputePeriod;
    }

    struct HTLC {
        address sender;
        address receiver;
        uint256 amount;
        bytes32 hashLock;
        uint256 timelock;
        bool isSettled;
    }

    // Core functions
    function openChannel(address[] calldata participants) external payable returns (bytes32);
    function closeChannel(bytes32 channelId, bytes32 finalState) external;
    function updateChannelState(bytes32 channelId, bytes32 newState) external;
    function createHTLC(bytes32 channelId, address receiver, uint256 amount, bytes32 hashLock) external;
    function resolveHTLC(bytes32 channelId, bytes32 htlcId, bytes32 preimage) external;
    function getChannelState(bytes32 channelId) external view returns (ChannelState memory);
    function getHTLC(bytes32 htlcId) external view returns (HTLC memory);
}