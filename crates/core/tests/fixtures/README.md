These two PEMs are one disposable RSA test key in PKCS#1 and PKCS#8 formats.
They have never been registered with GitHub or used outside tests. They provide
a fixed conversion vector and let the Worker integration tests verify real
WebCrypto signatures without network access. Regenerate only as a pair:

```sh
openssl genrsa -traditional -out github-app-test-rsa.pem 2048
openssl pkcs8 -topk8 -nocrypt -in github-app-test-rsa.pem -out github-app-test-pkcs8.pem
```
