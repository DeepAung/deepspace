# DeepSpace

A multiplayer spaceship game built with **[Rust](https://www.rust-lang.org/)** and **[Bevy Game Engine](https://bevyengine.org/)**.

This project serves as a practical implementation of an **authoritative server** architecture in a real-time game environment.

> **Inspiration**
> This project is heavily inspired by Gabriel Gambetta's excellent series on [Fast-Paced Multiplayer](https://gabrielgambetta.com/client-server-game-architecture.html).

<img width="1920" height="1080" alt="Screenshot_20260319_220505" src="https://github.com/user-attachments/assets/ca944ee4-9f34-48d8-92cb-46b40fa4b4ad" />

## Key Features

* **Authoritative Server**: The server controls the state of the game.
* **Client-Side Prediction**: The client predicts its own state locally, making the game feel responsive immediately without waiting for the server.
* **Server Reconciliation**: The client seamlessly corrects its local state if it diverges from the server's authoritative state.
* **Entity Interpolation**: Smooths out the movement of other entities (ships and bullets).

## Project Structure

The project is divided into three main crates.

* **`server/`**: The dedicated game server that handles physics, collisions, and state authority.
* **`client/`**: The player's game client, handling rendering, input, and prediction.
* **`shared/`**: Common logic, protocol definitions, and game components shared between client and server.

## Setup & Installation

### Prerequisites

* **Rust**: [Install Rust](https://www.rust-lang.org/tools/install).
* **Make**: Used for running convenience scripts.
* **Bacon** (Optional): A background code checker for Rust. Useful for auto-restarting during development. [Install Bacon](https://github.com/Canop/bacon).

### Running the Project

You can start the server and client using the provided `Makefile` commands.

**1. Start the Server**

```bash
make run-server
```

**2. Start the Client**

```bash
make run-client
```

To test multiplayer on a single machine, you can open multiple terminal windows and run `make run-client` in each one.

## Controls

* Rotate player using **mouse position**.
* **W** and **S** to move ship forward/backward.
* **Left-click** to shoot a bullet.
