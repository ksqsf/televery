# Televery

Televery is a hassle-free 2-step verification service, or more
accurately, server.

Televery server will listen for local verification requests via UNIX
domain socket, and generates a random number as verification number on
demand. Then, the Televery server will send a message containing this
number to your associated Telegram account.

Later, there might be even simpler methods. For example, you just
click a button "Confirm" and get it done.
