---
title: Set memlock limit
date: 2021-11-26 14:00:00
author: ihciah
---

# Set memlock limit
Io-uring needs to share memory in user mode and kernel mode, such as ring or registered buffer.

The default configuration of many kernels will have a small memlock limit number, such as 64(64 KiB). We need a larger memlock to work properly (if you manually specify the size of the ring, you may need to make sure that its size is legal).

To view the current limit, you can use `ulimit -l` (if you have just modified the configuration, you need to log in to the session again to take effect):
```
❯ ulimit -l
unlimited
```

To modify this limit globally, you can modify the `/etc/security/limits.conf` file and add two lines:
```
* hard memlock unlimited
* soft memlock unlimited
```

If you only want to take effect for this session, you can consider using the root user to execute `ulimit -Sl unlimited && ulimit -Hl unlimited your_cmd`.

In systemd, you can set the memlock limit through the `LimitMEMLOCK` configuration, you can refer to `/etc/systemd/user.conf` and `/etc/systemd/system.conf`.

Except unlimited, setting 512 is generally sufficient under normal circumstances. But if the system throughput is high, you can consider configuring a larger limit (or unlimited) and specify a larger number of ring entries when creating the runtime to obtain better performance.

What is the result when memlock is not enough? Sometimes an error will be returned when you read or write: `code: 105, kind: Uncategorized, message: "No buffer space available"`.
