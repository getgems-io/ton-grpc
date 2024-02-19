# Changelog

## [0.16.2](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.16.1...tonlibjson-client-v0.16.2) (2024-02-19)


### Bug Fixes

* **deps:** bump config from 0.13.4 to 0.14.0 ([a1f0c3d](https://github.com/getgems-io/ton-grpc/commit/a1f0c3d85e6d358a67665ad157f40fbfa566e1b5))
* **deps:** bump metrics from 0.22.0 to 0.22.1 ([d149272](https://github.com/getgems-io/ton-grpc/commit/d149272f9a9a715ad3d24687b44ebf2114731fd0))
* **deps:** bump syn from 2.0.48 to 2.0.49 ([8f64949](https://github.com/getgems-io/ton-grpc/commit/8f64949a1a1d21cfe2230136e1bac8f60334baf3))
* **deps:** bump thiserror from 1.0.56 to 1.0.57 ([b09f1ae](https://github.com/getgems-io/ton-grpc/commit/b09f1aea936e5e52f9f12707b70208386642ddda))

## [0.16.1](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.16.0...tonlibjson-client-v0.16.1) (2024-01-31)


### Bug Fixes

* send request to latest ls when route is unknown ([434bc28](https://github.com/getgems-io/ton-grpc/commit/434bc28ab22a025ff03bf93b92f47295d11df835))

## [0.16.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.15.0...tonlibjson-client-v0.16.0) (2024-01-29)


### Features

* support host resolving from config ([f1850b7](https://github.com/getgems-io/ton-grpc/commit/f1850b78ba976a86debd137a31892748ece3caf8))

## [0.15.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.14.1...tonlibjson-client-v0.15.0) (2024-01-24)


### Features

* wait for not available block ([da4047d](https://github.com/getgems-io/ton-grpc/commit/da4047d0e421da0873f11e0f4bfaa08813845bb4))
* watch masterchain block availability ([c29d672](https://github.com/getgems-io/ton-grpc/commit/c29d67215e916118dc44d597f293f33d5b693866))


### Bug Fixes

* **deps:** bump uuid from 1.6.1 to 1.7.0 ([5249d97](https://github.com/getgems-io/ton-grpc/commit/5249d97a8be9dfcaa2dcb25476c8c6e4de097b3f))
* drop internal caches ([24db23b](https://github.com/getgems-io/ton-grpc/commit/24db23bd98a2b7c613a779be2a02ef91cf933c8f))
* drop waitable ([ad9035e](https://github.com/getgems-io/ton-grpc/commit/ad9035e7f160d45544912218fce1fc1fcd23ae74))

## [0.14.1](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.14.0...tonlibjson-client-v0.14.1) (2024-01-22)


### Bug Fixes

* **deps:** bump metrics from 0.21.1 to 0.22.0 ([4fa411a](https://github.com/getgems-io/ton-grpc/commit/4fa411ab11d9db701cb40619801dfcec3b853853))
* **deps:** bump serial_test from 2.0.0 to 3.0.0 ([4a19742](https://github.com/getgems-io/ton-grpc/commit/4a19742d1ffd4b4ca5d0ceaa8222eb6004f9af7a))

## [0.14.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.13.1...tonlibjson-client-v0.14.0) (2024-01-19)


### Features

* k8s dns resolv ([76ffcb1](https://github.com/getgems-io/ton-grpc/commit/76ffcb14c593b13fa493d280841d9e6aae031070))


### Bug Fixes

* **deps:** bump prettyplease from 0.2.15 to 0.2.16 ([10872a4](https://github.com/getgems-io/ton-grpc/commit/10872a45e3820b5a4628cdc8fd7284cc858e7e92))
* **deps:** bump syn from 2.0.39 to 2.0.41 ([ca7e9db](https://github.com/getgems-io/ton-grpc/commit/ca7e9db737b79d6b5b30a0f147b8bb80fe70ea25))
* optional in_msg ([f1f1b2e](https://github.com/getgems-io/ton-grpc/commit/f1f1b2ecdc6ac8c3fe79a6aae26c6687e4f2b312))

## [0.13.1](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.13.0...tonlibjson-client-v0.13.1) (2023-12-12)


### Bug Fixes

* always fallback to latest servers ([#593](https://github.com/getgems-io/ton-grpc/issues/593)) ([5f7f0e2](https://github.com/getgems-io/ton-grpc/commit/5f7f0e28e63edf38e4453148ecc6598c6e8b9682))

## [0.13.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.12.2...tonlibjson-client-v0.13.0) (2023-12-07)


### Features

* Get all txs from block grpc method ([#580](https://github.com/getgems-io/ton-grpc/issues/580)) ([69f4d46](https://github.com/getgems-io/ton-grpc/commit/69f4d46b3d39f7672c3037000c2067ac4dc834a7))

## [0.12.2](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.12.1...tonlibjson-client-v0.12.2) (2023-12-06)


### Bug Fixes

* add fallback for router ([b663485](https://github.com/getgems-io/ton-grpc/commit/b6634850081689186e7293a6607bfea922e65bf6))

## [0.12.1](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.12.0...tonlibjson-client-v0.12.1) (2023-11-29)


### Bug Fixes

* broken deseriealization ([86139be](https://github.com/getgems-io/ton-grpc/commit/86139bee7b454bc8c8efe544330be93a03b3a859))

## [0.12.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.11.0...tonlibjson-client-v0.12.0) (2023-11-27)


### Features

* TL parser and struct generator ([#555](https://github.com/getgems-io/ton-grpc/issues/555)) ([a4392d2](https://github.com/getgems-io/ton-grpc/commit/a4392d2e66223157a8af6dc48ab266aef7a6773f))


### Bug Fixes

* **deps:** bump uuid from 1.5.0 to 1.6.1 ([87c559f](https://github.com/getgems-io/ton-grpc/commit/87c559f10d1170bf7918a2255a989080676ae099))

## [0.11.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.10.0...tonlibjson-client-v0.11.0) (2023-11-15)


### Features

* Graceful handling for next block request ([#530](https://github.com/getgems-io/ton-grpc/issues/530)) ([a045855](https://github.com/getgems-io/ton-grpc/commit/a04585558768c222cb9e05b3d4671c18a53f8878))


### Bug Fixes

* **deps:** bump itertools from 0.11.0 to 0.12.0 ([fe22337](https://github.com/getgems-io/ton-grpc/commit/fe22337562d77688003814f2b269201ed6723e1b))

## [0.10.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.9.0...tonlibjson-client-v0.10.0) (2023-11-14)


### Features

* retry metrics ([cb7bf9d](https://github.com/getgems-io/ton-grpc/commit/cb7bf9d7ea93cc9f1b74a43a519f267d44285e3a))

## [0.9.0](https://github.com/getgems-io/ton-grpc/compare/tonlibjson-client-v0.8.0...tonlibjson-client-v0.9.0) (2023-11-08)


### Features

* drop from_env ([9baddec](https://github.com/getgems-io/ton-grpc/commit/9baddec6cbdd59225df33c3955eca93851f712af))
* ewma configurable ([e20bb2d](https://github.com/getgems-io/ton-grpc/commit/e20bb2d63e0e77ebe9379c3aae1f1d877d87e054))
* ton client builder ([a4ff592](https://github.com/getgems-io/ton-grpc/commit/a4ff592a501ffd997821c178ac37150432459f81))
* ton-grpc ton config url ([6dadf81](https://github.com/getgems-io/ton-grpc/commit/6dadf8123397796d150183cee94b3363667a174d))
* tonclient timeout layer ([fc3ec4d](https://github.com/getgems-io/ton-grpc/commit/fc3ec4defe858c060f9e6c83b10effef37c33e09))


### Bug Fixes

* **deps:** bump derive-new from 0.5.9 to 0.6.0 ([218e91b](https://github.com/getgems-io/ton-grpc/commit/218e91bcdc19444d48c2de5f9d79b04be43eb640))

## [0.8.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.7.0...tonlibjson-client-v0.8.0) (2023-11-06)


### Features

* GetAccountStateRequest at_least_block_id criteria ([b37a0a0](https://github.com/getgems-io/tonlibjson/commit/b37a0a0f17dd170c50623a10a3f7dc20f31c5ef7))
* GetShardAccountCellRequest at_least_block_id criteria ([4ea3004](https://github.com/getgems-io/tonlibjson/commit/4ea3004b70cf2d3904a52b59de3f4753198860e5))

## [0.7.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.6.0...tonlibjson-client-v0.7.0) (2023-11-03)


### Features

* set up timeuts & delay between retries ([223f55d](https://github.com/getgems-io/tonlibjson/commit/223f55d1e1fa8b2cfdf630aa3e066b69acbb071d))

## [0.6.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.5.0...tonlibjson-client-v0.6.0) (2023-11-03)


### Features

* add router miss metric ([ce35f5f](https://github.com/getgems-io/tonlibjson/commit/ce35f5f41a166795ac3db5beb60f9ef1f60f6c87))

## [0.5.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.7...tonlibjson-client-v0.5.0) (2023-11-01)


### Features

* support multiple shards ([#518](https://github.com/getgems-io/tonlibjson/issues/518)) ([b76067f](https://github.com/getgems-io/tonlibjson/commit/b76067fa3a02566ac85a8e959ab2adf9a09d5978))

## [0.4.7](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.6...tonlibjson-client-v0.4.7) (2023-10-27)


### Bug Fixes

* dont do extra request ([5b692bd](https://github.com/getgems-io/tonlibjson/commit/5b692bde4869e63cb767262f0b57f5ceffc24513))

## [0.4.6](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.5...tonlibjson-client-v0.4.6) (2023-10-27)


### Bug Fixes

* dont wait for archive node ([cbc7b64](https://github.com/getgems-io/tonlibjson/commit/cbc7b64a6202a2589a566b9441fa690a1e68f5f4))

## [0.4.5](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.4...tonlibjson-client-v0.4.5) (2023-10-25)


### Bug Fixes

* **deps:** bump uuid from 1.4.1 to 1.5.0 ([f1b5cd6](https://github.com/getgems-io/tonlibjson/commit/f1b5cd670241eaf9fb1178aabd338977f2d73f53))


### Performance Improvements

* request masterchain info only one time per 4 hour ([6eaa33d](https://github.com/getgems-io/tonlibjson/commit/6eaa33dd4e0bd64c42eca1315d95864a563e565b))

## [0.4.4](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.3...tonlibjson-client-v0.4.4) (2023-10-18)


### Performance Improvements

* optimize fething headers of last block ([9d235eb](https://github.com/getgems-io/tonlibjson/commit/9d235eb60aa983838fe33fddfe928c35f06cb1f8))

## [0.4.3](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.2...tonlibjson-client-v0.4.3) (2023-10-18)


### Performance Improvements

* more optimal way to check first block expires ([aa045b0](https://github.com/getgems-io/tonlibjson/commit/aa045b0b3a20911ece3e0a4dd1c2442e886aba7c))

## [0.4.2](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.1...tonlibjson-client-v0.4.2) (2023-10-17)


### Performance Improvements

* lookup block cache ([e3fc268](https://github.com/getgems-io/tonlibjson/commit/e3fc268b104476c18edb5073346cbc04602fdd1d))

## [0.4.1](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.4.0...tonlibjson-client-v0.4.1) (2023-10-16)


### Bug Fixes

* use internal cache for get_shards ([96095c7](https://github.com/getgems-io/tonlibjson/commit/96095c746468379cfd3f886a1ccdcbb51dd1e1ce))

## [0.4.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.3.1...tonlibjson-client-v0.4.0) (2023-10-16)


### Features

* total count of reqs ([#490](https://github.com/getgems-io/tonlibjson/issues/490)) ([ed91cba](https://github.com/getgems-io/tonlibjson/commit/ed91cbaeff66a75ffa75729019cdbbb60adbe60a))


### Bug Fixes

* **deps:** bump async-trait from 0.1.73 to 0.1.74 ([be6dac7](https://github.com/getgems-io/tonlibjson/commit/be6dac75b6db9aa727f9877b6bd682744c3d7746))

## [0.3.1](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.3.0...tonlibjson-client-v0.3.1) (2023-10-13)


### Bug Fixes

* fix requests metric ([f0a0aab](https://github.com/getgems-io/tonlibjson/commit/f0a0aabf32064da744185987e31284b3c7d4dd3e))

## [0.3.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.2.0...tonlibjson-client-v0.3.0) (2023-10-13)


### Features

* liteserver requests ([a69b2b8](https://github.com/getgems-io/tonlibjson/commit/a69b2b86c73862f1ad4ee66d5511e73921de6801))

## [0.2.0](https://github.com/getgems-io/tonlibjson/compare/tonlibjson-client-v0.1.0...tonlibjson-client-v0.2.0) (2023-10-12)


### Features

* add some metrics to cursor client ([d6eb358](https://github.com/getgems-io/tonlibjson/commit/d6eb3583482414e040852ed2f20de92a81c4c9ae))

## 0.1.0 (2023-10-11)


### Features

* ton-grpc ton_liteserver_last_seqno metric ([b952f53](https://github.com/getgems-io/tonlibjson/commit/b952f533ae0e795f46efb37c725ebf25a52d4d71))


### Bug Fixes

* ton-grpc helm chart metrics arg ([04dbced](https://github.com/getgems-io/tonlibjson/commit/04dbcede350a32dccbd529e180f242343cabb1d8))
