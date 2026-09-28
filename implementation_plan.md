# 环形菜单缩放修复（#85）与女仆模型持久化及交互重构（#91）实施计划

## 1. 目标概述
本项目在 NeoForge 1.21.1 平台下排查并解决两个关键体验与功能缺陷：
1. **GitHub Issue #85**：环形菜单（Radial Menu / WheelScreen）在视频设置调整界面缩放比率（GUI Scale）时出现文字严重溢出穿插、相邻扇区重叠、以及所有扇区被暴力裁剪为 `gui.mmdskin.confi..` 的缺陷；
2. **GitHub Issue #91**：车万女仆（Touhou Little Maid）兼容模型在游戏重启后配置完全丢失恢复默认的问题，并在用户提议的基础上，实现“原生背包界面换装按钮注入”与“在主轮盘中集成以玩家为主人的女仆列表管理界面”的双轨直观交互系统。

---

## 2. 方案技术架构与改动清单

### 模块一：环形菜单自适应与文字动态缩放（解决 Issue #85）

#### 2.1 扇区几何与文字测量解耦
- **文件**：`common/src/main/java/com/shiroha/mmdskin/ui/wheel/AbstractWheelScreen.java`
- **问题根源**：原实现中使用 `Mth.clamp(..., 86.0f, 210.0f)` 强行设定了 86 像素的保底宽度。在较大 GUI Scale 或小逻辑分辨率下，扇区真实物理弦长仅 30~50 像素，导致 86 像素的文字侵入邻近扇区达 20 像素以上造成严重重叠错位；而 `fitText` 暴力削减字符直到宽度小于 86 像素，又导致以 `gui.mmdskin.config.` 开头的长文本全部被一致截断为 17 字符的 `gui.mmdskin.confi..`。
- **改动设计**：
  1. 计算扇区真实的几何安全宽度 `safeWidth = 2.0f * textRadius * (float) Math.sin(halfSweepRad) * 0.88f`，彻底移除人为强加的 86px 最小硬限制。
  2. 引入 `PoseStack` 矩阵等比缩放：当文字宽度 `textWidth > safeWidth` 时，动态计算缩放系数 `scale = Math.min(1.0f, safeWidth / (float) textWidth)`。设定缩放下限为 `0.62f`，只要缩放后能够容纳，则保持文字完整不截断；仅当缩放至 0.62x 后仍超出可用空间时，才调用放宽后的 `fitText` 进行末尾字符省略。
  3. 修正中心气泡尺寸计算：避免 `bubbleRadius` 保底 30 像素导致的溢出外圈，确保气泡半径不超过 `innerRadius - 2`。
  4. 将 `WheelEntry` 升级为支持 `Component`，并在绘制时动态测量与本地化渲染，彻底消除构造函数过早固化导致原始未翻译键名的问题。

---

### 模块二：女仆模型持久化与数据自愈（解决 Issue #91 根源）

#### 2.2 本地持久化与按需自愈
- **文件**：`common/src/main/java/com/shiroha/mmdskin/ui/config/ModelSelectorConfig.java`
- **改动设计**：
  1. 在 `ConfigData` 中新增 `maidModels` 映射字典（`ConcurrentHashMap<String, String>`），以女仆实体的全局唯一 `UUID` 字符串为键，以 MMD 模型名称为值。
  2. 提供 `getMaidModel(UUID)`、`setMaidModel(UUID, String)`、`removeMaidModel(UUID)` 和 `getAllMaidModels()` 接口。设置或清除时立即保存至磁盘配置文件 `config/mmdskin/model_selector.json`。

- **文件**：`common/src/main/java/com/shiroha/mmdskin/compat/maid/runtime/MaidMMDModelManager.java`
- **改动设计**：
  1. 在 `bindModel(UUID, modelName)` 中不仅维护内存缓存，同时同步回写 `ModelSelectorConfig`。
  2. 在 `unbindModel(UUID)` 中同步从 `ModelSelectorConfig` 移除。
  3. 重构 `getBindingModelName(UUID)` 与 `hasMMDModel(UUID)`：当内存缓存未命中时，自动检索 `ModelSelectorConfig` 本地配置。若配置中存在有效模型，自动回填进内存并初始化，使得世界重载或客户端重启后，女仆一进入视距便能无缝自愈恢复模型。

#### 2.3 网络同步协议与服务端持久化注册
- **文件**：`common/src/main/java/com/shiroha/mmdskin/compat/maid/network/MaidModelNetworkHandler.java`
- **文件**：`neoforge/src/main/java/com/shiroha/mmdskin/neoforge/network/MmdSkinNetworkPack.java`
- **改动设计**：
  1. 在 `MAID_MODEL` 网络包中，载荷格式升级为 `maidUUID.toString() + "|" + modelName`，向后兼容保留 `entityId`。
  2. 客户端解包时优先基于 UUID 执行绑定，彻底消除实体跨视距生成延迟导致的绑定丢失。
  3. 服务端在收到女仆模型更新时，在服务端模型表中常驻登记，供新玩家发送 `REQUEST_ALL_MODELS` 时统一回传。

---

### 模块三：女仆直观交互系统重构（落实用户指定的直观方案）

#### 3.1 车万女仆原生背包界面换装按钮注入
- **文件**：`neoforge/src/main/java/com/shiroha/mmdskin/neoforge/maid/MaidContainerGuiHandler.java`
- **改动设计**：
  1. 订阅客户端界面初始化事件 `ScreenEvent.Init.Post`。
  2. 当当前打开的界面为车万女仆的原生背包容器界面（`AbstractMaidContainerGui` 或包含 `Maid` 的容器界面）时，通过容器获取当前交互的女仆实体实例 `EntityMaid`。
  3. 在女仆背包界面的合适位置（靠近左上角女仆头像与原生衣架按钮处）注入一个半透明质感的 MMD 快捷按钮。
  4. 玩家点击按钮时，直接弹出 `new MaidModelSelectorScreen(maid.getUUID(), maid.getId(), maid.getName().getString())`。
  5. 契合车万女仆原生交互习惯，零学习成本且天然具备主人权限保护。

#### 3.2 主轮盘接入主人女仆列表管理界面
- **文件**：`common/src/main/java/com/shiroha/mmdskin/ui/wheel/ConfigWheelScreen.java`
- **改动设计**：
  1. 在检测到车万女仆模组处于加载状态时，主配置轮盘中追加一个 `maid` 槽位，显示名称为“女仆管理”（本地化键 `gui.mmdskin.config.maid_manager`）。
  2. 点击该槽位唤出专门的主人女仆集中管理界面 `PlayerMaidManagerScreen`。

- **文件**：`common/src/main/java/com/shiroha/mmdskin/compat/maid/ui/PlayerMaidManagerScreen.java`（新建）
- **改动设计**：
  1. 检索当前世界中以当前玩家为主人的女仆实体（匹配 `getOwnerUUID()` 等同于玩家 UUID），同时合并本地 `ModelSelectorConfig` 中曾经记录过的所有女仆记录。
  2. 以卡片列表形式展示所有归属于玩家的女仆：显示女仆名称、当前配置的 MMD 模型名称、在线/视距状态。
  3. 提供直观的“更改模型”与“还原原版”按钮，点击即可直接进入该女仆的模型选择器。玩家无需在游戏世界中四处走动追逐女仆，即可统一调整所有女仆的模型。

#### 3.3 模型选择界面（MaidModelSelectorScreen）功能完善
- **文件**：`common/src/main/java/com/shiroha/mmdskin/compat/maid/ui/MaidModelSelectorScreen.java`
- **改动设计**：
  1. 列表首项固定显示为“恢复原版女仆外观”（Default），并以醒目图标标记。
  2. 界面顶部增加轻量级搜索过滤输入框 `EditBox`，输入关键词实时过滤模型列表。
  3. 当前正在使用的模型项右侧标注“当前使用”高亮标记。

---

## 3. 受影响文件清单一览
| 文件路径 | 修改性质 | 核心功能点 |
| :--- | :--- | :--- |
| `common/.../ui/wheel/AbstractWheelScreen.java` | 修改 | 文字自适应 PoseStack 缩放、移除 86px 硬限制、气泡尺寸防溢出、Component 支持 |
| `common/.../ui/wheel/ConfigWheelScreen.java` | 修改 | 适配 Component、追加女仆管理槽位 |
| `common/.../ui/wheel/MaidConfigWheelScreen.java` | 修改 | 适配 Component 与尺寸参数 |
| `common/.../ui/config/ModelSelectorConfig.java` | 修改 | 增加 maidModels 持久化字段及磁盘读写逻辑 |
| `common/.../compat/maid/runtime/MaidMMDModelManager.java` | 修改 | 绑定写入配置、未命中时自愈读取本地配置 |
| `common/.../compat/maid/service/DefaultMaidModelSelectionService.java` | 修改 | 协同持久化与网络同步 |
| `common/.../compat/maid/network/MaidModelNetworkHandler.java` | 修改 | 网络包载荷携带 UUID |
| `neoforge/.../neoforge/network/MmdSkinNetworkPack.java` | 修改 | 网络包支持 UUID 序列化解析与服务端回放 |
| `neoforge/.../neoforge/maid/MaidContainerGuiHandler.java` | 新增 | 监听背包界面并注入 MMD 换装按钮 |
| `neoforge/.../neoforge/register/NeoForgeClientRuntimeHooks.java` | 修改 | 注册 GUI 注入处理器与女仆快捷交互优化 |
| `common/.../compat/maid/ui/PlayerMaidManagerScreen.java` | 新增 | 主人专属女仆列表统一管理与批量配置界面 |
| `common/.../compat/maid/ui/MaidModelSelectorScreen.java` | 修改 | 增加搜索框、显式恢复原版项与当前模型标记 |
| `common/.../assets/mmdskin/lang/` | 修改 | 补充女仆管理、搜索框及操作提示的多语言条目 |

---

## 4. 实施与验证步骤
1. **基础库与持久化改造**：实现 `ModelSelectorConfig` 的女仆映射与 `MaidMMDModelManager` 的自愈读取。
2. **环形菜单缩放修复**：重构 `AbstractWheelScreen` 中的几何安全弦长与 `PoseStack` 文字缩放逻辑，并验证在不同 GUI 缩放比率下的渲染表现。
3. **原生背包界面注入**：编写 `MaidContainerGuiHandler`，在车万女仆背包界面中注入半透明快捷按钮。
4. **主人女仆管理界面**：编写 `PlayerMaidManagerScreen` 并在主轮盘中完成入口接入。
5. **构建与运行测试**：执行 `./gradlew compileJava` 验证编译无误，测试单人世界与重启场景下的模型持久性。
