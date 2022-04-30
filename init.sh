git submodule update --init --recursive
cmake .. -DCMAKE_BUILD_TYPE=Release -DOPENSSL_ROOT_DIR=/usr/local/opt/openssl
cmake --build . -j$(nproc) --target tonlibjson

OPENSSL_ROOT_DIR=/usr/local/opt/openssl
