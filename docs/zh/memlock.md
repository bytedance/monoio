---
title: 设置 memlock 限制
date: 2021-11-26 14:00:00
author: ihciah
---

# 设置 memlock 限制
io-uring 需要在用户态和内核态共享内存，如 ring 或 registered buffer。

很多内核的默认配置会带一个数值较小的 memlock 限制，如 64（指 64 KiB）。我们需要更大的 memlock 才能正常工作（如果你手动指定了 ring 的大小，你可能需要确保它的大小是合法的。我们测试中，64 其实也是能跑的）。

要查看当前的限制，可以使用 `ulimit -l`（如果你刚刚修改完配置，需要重新登录会话才会生效）:
```
❯ ulimit -l
unlimited
```

要全局修改这个限制，可以修改 `/etc/security/limits.conf` 文件，添加 2 行内容（你也可以把星号换成你的用户名）:
```
*    hard    memlock        unlimited
*    soft    memlock        unlimited
```

如果你只希望对本会话生效，可以考虑使用 root 用户执行 `ulimit -Sl unlimited && ulimit -Hl unlimited your_cmd`。

在 systemd 中，可以通过 `LimitMEMLOCK` 配置来设置 memlock 限制，可以参考 `/etc/systemd/user.conf` 和 `/etc/systemd/system.conf`。

除了 unlimited 外，正常情况下设置 512 一般是足够的。但如果吞吐量较高，可以考虑配置更大的限制（或 unlimited）并在创建 Runtime 时指定更大的 ring entry 数以获得更好的性能（参考 configuration.md）。

memlock 不够时的表现是什么？有时候在你读写时会返回错误：`code: 105, kind: Uncategorized, message: "No buffer space available"`。
