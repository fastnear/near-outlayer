#!/bin/bash
# Get collateral from worker logs (copy JSON block)

near contract call-function as-transaction dao.outlayer.near update_collateral \
  json-args "$(jq -n --arg c "$(cat collateral.json)" '{collateral: $c}')" \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as owner.outlayer.near \
  network-config mainnet \
  sign-with-legacy-keychain \
  send

near contract call-function as-transaction worker.outlayer.near update_collateral \
  json-args "$(jq -n --arg c "$(cat collateral.json)" '{collateral: $c}')" \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as owner.outlayer.near \
  network-config mainnet \
  sign-with-legacy-keychain \
  send  
