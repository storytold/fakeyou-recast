#!/bin/bash

declare -r CRYPTO_SECRET="FAKEYOU_DEVELOPMENT"

declare -r name_path="morgan-freeman"
declare -r name_full="Morgan Freeman"

cargo run \
  --bin  model-packager --release \
  -- \
  --crypto-secret "${CRYPTO_SECRET}" \
  --acoustic-file "./models/${name_path}.recast" \
  --vocoder-file "./models/${name_path}-hifigan-jit.pt" \
  --model-name "${name_full}" \
  --model-token "vcm_ABCDEF012345" \
  --author-username "Vegito" \
  --expires-in-days 30 \
  --output-file "./models/${name_path}.fakeyou"
