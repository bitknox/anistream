# The published store image, for render.yaml. Render mounts a service's disk owned by root and
# has no setting for the user a service runs as, so on Render the store runs as root to be able to
# write to /data. Everything else is the image as published.
FROM ghcr.io/bitknox/anistream-store:1
# hadolint ignore=DL3002
USER 0:0
