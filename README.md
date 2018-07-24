# Televery

Televery is a hassle-free 2-step verification service, or more
accurately, server.

1. Televery server listens for verification requests via a TCP socket.
2. Some client asks for verification.
3. Televery sends you a verification message, which comprises
   essential information and two inline buttons "Pass" and "Deny".
   Meanwhile, the client waits for reply from Televery.
4. If you click "Pass", the client will be noticed that the
   verification succeeded.
5. If you click "Deny" or it's timed out, the client will be noticed
   that the verification failed.

Televery is very user-friendly and easy-to-use. It just needs you to
incorporate it into your systems.
