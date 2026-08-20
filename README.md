# SMMDB API

[![Continuous integration](https://img.shields.io/travis/Tarnadas/smmdb-api?label=Travis%20CI&logo=travis)](https://travis-ci.org/github/Tarnadas/smmdb-api)
[![Discord](https://img.shields.io/discord/168893527357521920?label=Discord&logo=discord&color=7289da)](https://discord.gg/SPZsgSe)
[![Twitter](https://img.shields.io/twitter/follow/marior_dev?style=flat&logo=twitter&label=follow&color=00acee)](https://twitter.com/marior_dev)

API server for [SMMDB](https://smmdb.net).

The API's documentation can be visited here: https://api.smmdb.net
Please keep in mind, that it is not yet well documented and I am waiting for a way to do this automatically from code.

## Self-hosting

The original SMMDB site is shut down, so all runtime configuration is environment-driven
(`MONGODB_URI`, `GOOGLE_CLIENT_ID`, `COOKIE_SECRET`, `PORT`). See
[`docs/SELFHOSTING.md`](docs/SELFHOSTING.md) for full instructions, including how to
sanitize a shared backup of the original database (`accounts` and `amazon` collections
contain personal data and embedded AWS keys and are dropped).
