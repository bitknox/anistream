# The published store image, for heroku.yml, so Heroku pulls it rather than compiling Rust.
# Heroku runs an image's CMD through /bin/sh, which this image does not have, and runs ENTRYPOINT
# as given, so the whole command goes in ENTRYPOINT. Setting it also clears the inherited CMD.
FROM ghcr.io/bitknox/anistream-store:2
ENTRYPOINT ["anistream-store", "serve"]
