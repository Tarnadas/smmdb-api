FROM debian:buster-slim

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
    pkg-config \
    openssl \
    libssl-dev \
    iproute2 \
    ; \
    \
    rm -rf /var/lib/apt/lists/*;

COPY --from=tarnadas/smmdb-api-build /binary ./smmdb

ENV DOCKER=true
EXPOSE 3030

CMD ["./smmdb"]
