# Rust RTI: Lingua Franca Runtime Infrastructure (RTI) in Rust Language

This repository contains the Rust code for Lingua Franca's (LF) Runtime Infrastructure (RTI) for federated execution of LF.

> **_Disclaimer_**
>
> This RTI is still a work in progress with unimplemented functionalities; thus, it may not work for certain federated LF programs.
> Please let @chanijjani or @hokeun know if you find any issues when running federated LF programs with this Rust RTI.

## Requirements
- Rust runtime: https://www.rust-lang.org/tools/install

## Quick Start

TODO

- Change the directory into `rust/rti`, then run the `cargo` command for running the RTI, as shown below.

```
cd rust/rti
cargo run -- -n 2
```

## Current Status

- Passing federated tests with Rust RTI: SimpleFederated.lf, StopAtShutdown.lf, DistributedCount.lf, DistributedStop.lf, PingPongDistibuted.lf