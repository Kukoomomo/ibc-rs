#!/usr/bin

#docker build -t $(IMAGE_NAME) .

docker run -d --name ibcrelayer -it hermes:local /bin/bash

docker cp ./workspace/ $(docker ps -q -f name=ibcrelayer):/root/.hermes/

docker exec -it $(docker ps -q -f name=ibcrelayer) /bin/bash -c `hermes start`