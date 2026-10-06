# Third-party notices

This wallet builds on the work of others. Their licenses apply to the parts named below.

## dogecoin-vm-wallet

The wallet's design (a Rust core held to the web wallet's vectors, review before signing, a
hardware-protected vault) and parts of the core's code follow
[paulgnz/dogecoin-vm-wallet](https://github.com/paulgnz/dogecoin-vm-wallet).

```
MIT License

Copyright (c) 2026 DogecoinVM contributors

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## BTCVM (btc-vm)

The core's transaction, address and bridge code ports the BTCVM web wallet's `chain.js`, and
`core/tests/vectors/wallet-vectors.json` is copied unchanged, from
[MetalBlockchain/btc-vm](https://github.com/MetalBlockchain/btc-vm). This wallet is independent: it is not
made or endorsed by Metallicus.

```
BSD 3-Clause License

Copyright (c) 2024-2026, Metallicus, Inc.
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## DogecoinVM (dogecoin-vm)

The core's Dogecoin code (`core/src/doge.rs`: legacy transactions, fees, the DVMO tag and the
P2SH deposit and peg addresses) ports DogecoinVM's web wallet `chain.js`, and
`core/tests/vectors/doge-wallet-vectors.json` is copied unchanged, from
[MetalBlockchain/dogecoin-vm](https://github.com/MetalBlockchain/dogecoin-vm). This wallet is independent: it
is not made or endorsed by Metallicus.

```
BSD 3-Clause License

Copyright (c) 2024-2026, Metallicus, Inc.
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## BIP 350 test vectors

`core/tests/addresses.rs` uses the segwit address test vectors of
[BIP 350](https://github.com/bitcoin/bips/blob/master/bip-0350.mediawiki) by Pieter Wuille, licensed under the
2-clause BSD license.

## BIP 39, BIP 32 and BIP 84

`core/src/bip39-english.txt` is BIP 39's English word list, unchanged (SHA-256
`2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda`), from
[BIP 39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki) by Marek Palatinus, Pavol Rusnak,
Aaron Voisine and Sean Bowe, licensed under the MIT License.
`core/tests/vectors/bip39-vectors.json` holds the English vectors of
[python-mnemonic](https://github.com/trezor/python-mnemonic) (MIT License, Copyright (c) 2013-2018 Pavol
Rusnak). `core/tests/vectors/bip32-vectors.json` holds test vectors 1 to 4 of
[BIP 32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki) by Pieter Wuille, licensed under the
2-clause BSD license, and `core/tests/seed.rs` checks the first address of
[BIP 84](https://github.com/bitcoin/bips/blob/master/bip-0084.mediawiki) (CC0-1.0).

## The Bitcoin logo

`app/ui/bitcoin.svg`, and its purple copy `app/ui/btcvm.svg`, are the Bitcoin logo, which is in the
public domain ([Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Bitcoin.svg)), as BTCVM's web
wallet ships it.
