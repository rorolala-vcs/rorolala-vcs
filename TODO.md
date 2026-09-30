# TODO

## `rola sync` —— 双向同步（设计与接口已确认，尚未实现）

一个 layout 通过本地 layout 里的 `TRACK` 绑定一个 vault，`rola sync` 只和它协作。
先根据「本地改动 × 本地锁定/所有权 × 远程现状」生成一份同步计划，再按计划执行。

### 模型

**只增，只约束上游**

- 远程 layout 的 uuid 永不删除。
- `@/removed/` 是可逆的废弃标记：文件移进去表示废弃，仍可取出，持有者仍可修改、仍可操作。
- 不存在彻底删除；没有墓碑、没有「管理员才能真删」。

**`@` 是远程 layout 的顶层保留前缀**

- `@` 本身只是一个特殊目录，其下记录逻辑分区。
- `@/new/<本地路径>@<uuid 短码>`：已提交、未定位。`@<uuid 短码>` 只为保证 path 唯一（Layout 里 path 与 uuid 一对一）。
- `@/removed/<…>`：已废弃。
- **定位** = 移出 `@/` 到正式路径（如 `Models/model.fbx`），并去掉 `@<uuid 短码>`；正式路径撞名只能拒绝或改名。
- 正式路径不能以 `@` 开头。
- 用 `@` 而不是 `#`：`#` 会被 bash/zsh 当作注释起始，`rola checkin #/new/x@abc --to-local y` 在交互式 shell 里会从 `#` 起整段被丢掉。
- **旧数据不兼容**：已经推上去的 `#/...` 条目需要迁移——逐条 `rola layout path move '#/new/x@abc' '@/new/x@abc'`（`#/removed/...` 同理）；新代码不再把 `#` 当标记，`#/...` 只是一条普通的正式路径。

**本地路径与上游解耦**

- 本地 layout 不引入 `@` 标记。
- 本地删除只是「我不要了」：移除本地条目，不影响上游，也不被 sync 传播。
- down 不改本地路径、不移动工作树文件。
- 本地远程操作成功后，顺手更新只读缓存（`.rola/cache/readonly-layouts/...`）。

**所有权**

- 以远程 layout 的 owner 为唯一权威。
- 管理员可移动任何人的文件；持有者动自己的；废弃持有者可做、管理员可代劳。

**管理员**

- 每个 vault 自己的 `vault.toml`：

  ```toml
  [auth]
  admins = []
  ```

- `rola vault admin add <name>` / `rola vault admin rm <name>` / 空 = 列出。
- 第一个管理员由人直接编辑配置写入。

### 命令

#### `rola sync [--up-only|--down-only|--both] [--force] [--dry-run] [--json] [--no-storage] [--no-index] [--no-layout]`

- 先出计划再执行；默认 `--both`。
- 只碰**已在本地 layout** 的 uuid；远程独有的 uuid 必须手动 `checkin` 才进本地 layout。
- 上行（up）：
  - 新 uuid（本地有、远程无）→ 远程 `@/new/<本地路径>@<uuid 短码>`，owner = 创建者。
  - 已有 uuid 且远程 owner 是本次账户 → 允许同步新版本；必须带「远程记录的基准版本 → 本地当前版本」整条链（index 对象 + store 内容）。
  - 已有 uuid 且非持有者 → 直接拒绝；sync 只报错并提示，三条出路（新 uuid 共享旧历史 / 只交变体等持有者合并 / 还原）本轮不做。
  - 基准链断了 → `--force` 强改历史（暂不审计），或日后 rebase。
  - 已废弃的 uuid：持有者仍可 push，路径仍在 `@/removed/` 下；恢复要显式移出。
  - `--no-layout`：不读不写远程 layout、跳过 layout 校验；新 uuid 只传 index/store、不建远程映射，等下次带 layout 的 sync 再建。
- 下行（down）：
  - 未改动 → 更新本地 layout 版本、只下载该版本的 store、切换工作树内容。
  - 已改动 → 整次 sync 中止（合并语义以后再做）。
  - `--no-layout`：不读远程 layout，按本地 layout 当前版本拉 store，index 全量。
- `--no-storage` / `--no-index` / `--no-layout` 跳过对应部分。
- 中途失败不保证原子：报告已做/未做，重跑 sync 收敛（幂等）。
- `--dry-run` / `--json` 的计划结构实现时再定。

#### `rola checkin <ref>... --to-local <path>...`

- 位置参数 Vec 与 `--to-local` Vec 等长配对，不等长报错（同 `rola track` 的逐文件 message 思路）。
- `ref` = 远程逻辑路径或 uuid；`--to-local` 是文件路径（不是目录）。
- 用途：把「远程有、本地无」的路径签入本地。
- 目标已存在则拒绝。

#### `rola layout path move`（扩展）

- 对 `--layout truth@vault` 写回远程 layout，而不是只改只读缓存。
- 用于定位、重命名、移入 `@/removed/`；发起者是持有者或管理员。

#### 废弃查询（扩展 `--layout truth@vault` 的查询族）

- 按 uuid 查，输出带 `deprecated`。
- 本地不对「上游废弃」做标记，只提供这个查询。

### 暂缓

- merge 语义
- audit（强改历史的审计记录）
- rebase（变更本地提交以满足正确版本链）
- 非持有者的三条出路
- `--dry-run` / `--json` 的计划结构
- `rola hold` / `rola giveup` 的 porcelain 接口（尚未设计）

### 已具备的前置

- `rola layout fetch` 把远程 layout 拉成只读缓存；`truth@vault` 可用于查询。
- 所有权 action（daemon id 10/11/12）与 `layout req-ownership` / `giveup-ownership` / `read-ownership` / `ls-ownership`。
- `storage sync-all` 与 `vcs-index sync-all` 传输 store / index。
- `layout tree-diff` 给出工作树 ↔ 本地 layout 的状态。
- `rola track` / `rola align` 把本地改动记进本地 layout。
- `./run.sh check` 已包含 i18n key 检查。
