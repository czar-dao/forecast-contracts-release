DeliverDAO Prediction Markets Contract
===============================

Prediction markets let users bet on the short time direction of a desired ticker

Contracts
---------

The source code for each contract is in the [`contracts/`](contracts/)
directory.

| Name                                               | Description                            |
| -------------------------------------------------- | -------------------------------------- |
| [`price-prediction`](contracts/price_prediction) | Central place managing positions and reward distribution (Opensource pending audit) |
| [`fast-oracle`](contracts/fast_oracle)       | Oracle for on-chain data |

Rust unit tests and compile contracts
--------
```
sh build_artifacts.sh
```

License
-------

Apache-2.0 License (see `/LICENSE`)
# forecast-contracts-opensourced
