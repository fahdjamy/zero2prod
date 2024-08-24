#!/usr/bin/env bash

curl -i -X POST -d 'email=thomasmann@hotmail.com&name=Tom' \
    http://127.0.0.1:8001/subscriptions

# to make this file executable, run cmd below in your terminal
# chmod +x scripts/new_subscriber.sh
