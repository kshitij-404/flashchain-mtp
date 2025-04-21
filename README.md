# FlashChain

A dual-layer blockchain sharding protocol for IoT networks, combining dynamic sharding with Lightning Network-inspired payment channels to enhance scalability and performance in supply chain applications.

## Introduction

FlashChain addresses critical challenges in integrating blockchain with IoT networks through a novel hybrid architecture. It significantly improves transaction throughput and reduces latency while maintaining security guarantees through:

1. A dual-layer sharding mechanism for adaptive scaling
2. A bridge layer connecting on-chain and off-chain operations
3. An optimized cross-shard communication protocol leveraging payment channels

This implementation demonstrates 1.5× higher transaction throughput and 70% lower cross-shard transaction latency compared to existing solutions.

## Tech Stack

- **Blockchain**: Ethereum-compatible (Solidity)
- **Smart Contracts**: Solidity
- **Off-chain Processing**: Rust
- **Development Tools**: Truffle, Ganache
- **Package Management**: Cargo (Rust), npm (JavaScript)

## Directory Structure

### Core

Contains the base blockchain layer implemented in Solidity:
- **base**: Foundational contracts and interfaces for sharding
- **consensus**: Implementation of modified PBFT consensus for sharded environments
- **governance**: Network parameter management and shard governance
- **sharding**: Core sharding functionality including management, registry, and routing
- **migrations**: Deployment scripts for smart contracts

### Lightning

Off-chain payment channel network implemented in Rust:
- **channel**: Channel management, operations, and state tracking
- **crypto**: Cryptographic functions for secure communications
- **network**: Peer-to-peer network management and topology
- **routing**: Path finding algorithms and payment handling
- **state**: State management for channels and network
- **benches**: Performance benchmarks

### Bridge

Connects the Core and Lightning layers:
- **contracts**: Solidity contracts for on-chain bridge operations
- **src**: Rust implementation for cross-layer state synchronization

### Common

Shared utilities and interfaces:
- **rust**: Common Rust utilities, configuration, and error handling
- **solidity**: Interfaces, libraries, and utilities for smart contracts

### Modules

Extensibility components:
- **rust**: Rust modules for building and state management
- **solidity**: Solidity modules for building and migration

## Installation

Currently only supports Rocky Linux and MacOS

### Prerequisites

1. **Node.js and npm**
   ```bash
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.2/install.sh | bash
   \. "$HOME/.nvm/nvm.sh"
   nvm install 22
   ```

2. **Rust and Cargo**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. **Truffle and Ganache**
   ```bash
   npm install -g truffle
   npm install -g ganache-cli
   ```

### Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/<yourusername>/flashchain.git
   cd flashchain
   ```

2. **Install JavaScript dependencies**
   ```bash
   npm install
   ```

### Usage

1. **Start a local blockchain**
   ```bash
   ganache-cli --deterministic --gas-limit 12000000
   ```

2. **Launch the script**
    ```bash
    ./modules/build/flashchain
    ```
