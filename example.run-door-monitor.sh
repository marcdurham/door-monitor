
cargo run -- \
  --sms-to-phone-number 2065552222 \
  --sms-api-username your@email.com \
  --sms-api-password yourPassword \
  --sms-from-phone-number 2065551111 \
  --api-url http://192.168.1.226/rpc/Input.GetStatus\?id\=0 \
  --check-interval-seconds 5 \
  --open-too-long-seconds 300 \
  --telegram-token "1111111111:AAAAAAAAAAAAAA" \
  --telegram-conversation-id 99999999999

