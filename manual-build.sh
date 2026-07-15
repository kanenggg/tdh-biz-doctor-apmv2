sbt consultation/docker:stage
date=$(date +%y%m%d%H%M%S)
tag="dev-$date"
artifactName="asia-southeast1-docker.pkg.dev/tdg-dh-truehealth-core-nonprod/cossack-docker/tdg-biz-apm-consultation"

docker build -t "$artifactName:$tag" consultation/target/docker/stage
docker push "$artifactName:$tag"