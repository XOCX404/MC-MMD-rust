# MC-MMD-Rust 项目架构与核心索引

本项目是一个将 MMD（MikuMikuDance）及 VRM 模型生态深度整合进 Minecraft 的高表现力客户端/服务端模组。工程采用 Architectury 多平台架构，上层对接现代 Minecraft 加载器生态（Fabric 与 NeoForge，并向下兼容 1.20.1 Forge 主干），底层深度集成基于 Rust 与 C++（Bullet3）自研的高性能原生物理与蒙皮计算引擎 `rust_engine`。通过 JNI 句柄路由与 Direct ByteBuffer 零拷贝管道，在保证 Minecraft 原版管线与光影兼容性的同时，实现了高帧率、高拟真的次时代角色渲染与布料物理表现。

---

## 1. 全局架构与技术全景

代码库自底向上划分为三个清晰的架构层级：最底层的原生计算核心、中间层的平台中立业务与渲染中枢，以及最外层的加载器特化适配层。

原生层由 `rust_engine` 构成，使用纯 Rust 编写并在构建期静态链接 Bullet3 物理引擎。它不仅负责零拷贝解析 PMX、VRM 等资产，还承担骨骼层级变换、CCD-IK 解算、多轨并行动画插值、Rayon 多线程 CPU 蒙皮与 GPU Compute 蒙皮数据供给。特别是在物理系统上，项目确立了严格的数学边界：骨骼系统、相机空间与 OpenGL 渲染运行于标准右手坐标系；Bullet3 物理模拟与 MMD 规范则运行于左手坐标系。两界交互通过对合矩阵变换（Involution）进行严格的无损映射，确保刚体世界姿态与局部约束锚点在往返转换时不产生手性反转。

中间业务层以 `common` 模块为核心，基于六边形架构（Hexagonal Ports）组织。它对下将繁杂的 JNI 调用拆解为细粒度的运行时端口，屏蔽原生层细节并支持无环境 Mock；对横向统一调度双分支渲染管线（CPU 蒙皮与 GPU 计算着色器蒙皮），实施 Blaze3D 渲染状态保护、第一人称拓扑动态裁切、多人联机模型同步以及显存预算治理；对上为各平台加载器提供统一的客户端生命周期调度与渲染委托。

外层加载器包含 `fabric`、`neoforge` 以及历史保留的 `forge` 模块。各端仅保留最小限度的平台特异性逻辑，利用 Architectury Loom 与 Shadow 构建流水线，将网络载荷、按键绑定、生物渲染接管与 Mixin 注入精准桥接至 `common` 中枢，实现了 90% 以上业务逻辑的跨平台零重复。

```
+---------------------------------------------------------------------------------+
|                                 Minecraft Client                                |
|  [Fabric Loader]                  [NeoForge Loader]             [Forge (Legacy)] |
|   - MmdSkinFabricClient            - MmdSkinNeoForgeClient       - MmdSkinForge  |
|   - FabricClientRuntimeHooks       - NeoForgeClientEventHandler  - ClientSetup   |
+---------------------------------------------------------------------------------+
                                         |
                                         v
+---------------------------------------------------------------------------------+
|                                 common (Java)                                   |
|  - bridge.runtime: NativeRuntimeBridge & 细粒度 Port 契约                        |
|  - render: OpenGlModelRenderer (CPU) / GpuSkinningModelRenderer (Compute Shader)|
|  - render.shader: ToonShader 色阶着色 / 倒置法线描边 / SSBOBindings 状态治理      |
|  - player: FirstPersonManager 动态视锥裁切 / 玩家独立模型替换 / 3层动画状态机    |
|  - compat: TaCZ 枪械快照姿态 / Iris 光影状态感知 / Vivecraft VR 追踪驱动         |
|  - asset/model: ModelRepository / ManagedModel 统一生命周期池                   |
+---------------------------------------------------------------------------------+
                                         |  JNI 句柄路由 & Direct ByteBuffer 零拷贝
                                         v
+---------------------------------------------------------------------------------+
|                            rust_engine (mmd_engine)                             |
|  - jni_bridge: native_func.rs (159个导出接口, 全局句柄映射与并发读写锁)           |
|  - physics: MMDPhysics / Bullet3 动力学调度 / 惯性速度注入 / 尾巴气动模型         |
|  - physics.topology: skirt_cross_joints 裙摆环向保形弹簧网络合成                |
|  - physics.collider: body_collider_synthesis 躯干跟骨碰撞体补全与推离保护       |
|  - model/skeleton: MmdModel 运行时实体 / Rayon 并发蒙皮 / CCD-IK 解算器         |
|  - bullet_wrapper: C++ Bullet3 C-ABI 安全封装静态库 (bw_api)                     |
+---------------------------------------------------------------------------------+
```

---

## 2. 原生引擎层：rust_engine 核心机制

`rust_engine`（Crate 名为 `mmd_engine`）对外编译为动态库 `cdylib` 供 Java JNI 调用，同时提供 `rlib` 支持桌面原生测试与探针工具。项目参考了 Cygames Gallop 引擎（赛马娘）在物理装配上的时序解耦哲学，杜绝在运动姿态下构建刚体锚点，从根本上解决了传统 MMD 物理在复杂移动中模型撕裂与爆炸的顽疾。

### 核心源码组织与职责
- `rust_engine/src/physics/mmd_physics.rs`：Bullet3 物理世界的总体调度器。统管物理刚体生命周期、每帧步进与状态同步。模拟在模型局部空间运行，每帧根据平滑后的世界移动速度反算惯性加速度，并为头发与尾巴注入二次迎风阻力与升力模型；步进后执行全局刚体线速度与角速度钳制，彻底消灭物理飞天现象。
- `rust_engine/src/physics/skirt_cross_joints.rs`：裙摆横向保形弹簧网络自动合成器。算法通过识别模型纵向裙片刚体，提取垂直父子链条并按拓扑深度分层；随后以几何中心计算方位角顺时针排序，在相邻链条间自动合成 6DOF 弹簧弦轴约束，有效防止裙摆在重力与运动中向内扁平塌陷或前后撕裂。
- `rust_engine/src/physics/body_collider_synthesis.rs`：躯干碰撞体智能补全与推离保护。针对缺少胸腔或臀部碰撞壳的模型，自动在对应骨骼位置植入胶囊体或球体刚体；在生成臀部碰撞体前严格核对现有后界边界，防止重复合成顶翻后裙摆。在回写骨骼前，执行几何推离检测，将深入体内的动态骨骼强行推回体表。
- `rust_engine/src/physics/mmd_rigid_body.rs` 与 `mmd_joint.rs`：刚体与关节参数数学映射。利用 `bone_offset` 矩阵实现刚体空间与骨骼空间的精准换算；引入 `STATIC_COLLISION_SHAPE_SCALE`（默认 0.70）收窄碰撞体半径，让衣物自然垂坠贴身；提供 `rebase_equilibrium` 动态校正弹簧零点，消除加载首帧回弹冲击。
- `rust_engine/src/physics/kinematic_target_filter.rs`：运动学目标静止死区滤波器。设定微小位移与旋转阈值，过滤角色待机时的浮点动画微噪，避免运动学刚体微振持续碰撞裙摆和长发。
- `rust_engine/src/model/runtime.rs`：模型运行时中枢 `MmdModel`。负责模型多材质分段管理、顶点缓冲区平铺排列、Rayon 多线程 CPU 蒙皮与 GPU 蒙皮矩阵供给，并统筹第一人称专用索引缓冲计算。
- `rust_engine/src/jni_bridge/native_func.rs`：JNI 本地方法集中导出点，提供 159 个跨语言交互函数。

### JNI 桥接规范与数据交互契约
Java 端与 Rust 端的内存安全依靠句柄路由机制保障。Rust 端使用 `RwLock<HashMap<i64, Arc<Mutex<MmdModel>>>>` 集中托管所有活跃对象，Java 侧仅持有 64 位整型句柄，杜绝跨语言裸指针误操作。对于高频的顶点数据、法线、骨骼矩阵与第一人称索引缓冲，严格使用 Direct ByteBuffer 通过堆外内存地址批量传输（`ptr::copy_nonoverlapping`），完全消除 JNI 数组逐项拷贝的性能瓶颈与 GC 压力。此外，`SetPhysicsConfig` 接口支持在游戏内热重载物理参数，当检测到重力或拓扑稳定配置变更时，自动驱动模型在下一渲染帧平滑重建物理世界。

### 自动化测试与基准探针矩阵
`rust_engine` 包含 251 项自动化单元测试，其中物理与几何模块独占 108 项测试，涵盖碰撞体补全断言、连通分量拓扑过滤、坐标对合变换与 VR 逆动力学解算。在 `examples/` 目录下部署了 `audit_pmx_physics.rs` 与 `probe_rin_physics.rs` 两大独立探针工具，可在无渲染环境下直接抓取真实模型在固定步长下的锚点误差与超限数据。项目根目录下持久化的 `grasswonder_audit.txt`、`rin_audit.txt` 等探针日志构成了物理回归测试的黄金基准数据集。

---

## 3. 通用核心层：common 业务与渲染中枢

`common` 模块构建在 Architectury 之上，屏蔽了底层平台细节，内聚了模组绝大部分的图形渲染、玩法机制与数据同步逻辑。

### 核心包组织划分
- `com.shiroha.mmdskin`：通用引导入口 `MmdSkin`、客户端总入口 `MmdSkinClient` 与跨平台原生库安全加载器 `NativeLibraryLoader`。
- `bridge.runtime`：接口隔离层。定义了 `NativeRuntimePort`、`NativeAnimationPort`、`NativeBoneOverridePort` 等细分接口，由 `NativeRuntimeBridge` 统一实现，在 JNI 边界前置完成非法浮点与畸变矩阵拦截。
- `render.backend`：统一模型实例抽象。下分 `render.backend.opengl`（基于版本脏标记更新的 CPU 蒙皮后端）与 `render.backend.gpu`（基于计算着色器的 GPU 蒙皮后端）。
- `render.shader`：赛璐珞渲染管线 `ToonShader`、计算着色器与 `SSBOBindings` 缓冲区状态治理。
- `player`：玩家交互核心。包含第一人称近视锥裁剪与双眼相机同帧求解（`FirstPersonManager`）、防走光评估（`AntiPeekEvaluator`）、独立模型替换与网络广播（`PlayerModelSelectionSyncService`）以及多层动画状态机（`AnimationStateManager`）。
- `asset` 与 `model.runtime`：模型文件扫描、异步加载协调器（`ModelLoadCoordinator`）与实体模型实例池（`ModelRepository`）。
- `compat`：外部生态软兼容层。反射适配 TaCZ 枪械瞄准姿态、Iris 光影感知、Vivecraft VR 追踪与车万女仆实体替换。

### 渲染管线与状态保护
模组实现了工业级的二阶段赛璐珞着色流程。第一阶段在主着色器中结合环境光照方向，将明暗计算为离散的色阶阶梯，并附加边缘光、高光染色与冷暖阴影；第二阶段基于倒置法线算法（Inverted Hull），开启正面剔除并将顶点沿法线向外微量扩张，精准勾勒出干净的二次元轮廓线。在 GPU 蒙皮路径下，蒙皮计算着色器在后台并行派发，结果直接作为 VBO 供 Blaze3D 绘制。为了消除频繁查询驱动状态引发的 CPU-GPU 同步停顿，`SSBOBindings` 在派发后直接清空解绑 0 到 15 号槽位。所有绘制调用均严格执行 Blaze3D 状态恢复契约，重置 VAO、剔除面、深度遮罩与混合模式，杜绝方块实体材质发黑或穿透。

### 第一人称与玩法关键解算
为彻底根治第一人称视角穿帮与物品栏联动缺陷，系统确立了场景隔离原则：第一人称模式仅作为单次渲染作用域（`RenderScene.FIRST_PERSON`），背包预览与纸娃娃在独立作用域运行，彼此互不干扰。在主视角下，系统采用“拓扑候选预计算 + 动态近视锥几何裁切”，Rust 端动态剔除落入近视锥的三角形并回填动态 EBO，在保证手部完整可见的同时消除了低头看脖颈的空心断口。同时，第一人称相机锚点取模型双眼几何中点，在 Minecraft 计算相机矩阵前提前驱动动画解算，消除了相机视点与模型画面之间的一帧时序延迟。三层动画状态机（基础机动层、动作交互层、姿态叠加层）使得角色在骑乘、鞘翅飞行、持枪开镜与潜行时均能平滑过渡。

---

## 4. 多平台适配层与构建体系

项目通过 Gradle 与 Architectury 构建矩阵，高效驱动跨平台构建与原生依赖打包。

### 构建体系与流水线（build.gradle）
根构建脚本自动化调度原生库的构建流转：首先自动检出指定 commit 的 Bullet3 源码；随后调用本地 Cargo 工具链执行 `cargo build --release` 编译 `rust_engine`；最后将编译产物归档至 `build/generated/native-resources/natives/<os-arch>/` 中并打包进 Jar。`settings.gradle` 统筹 `common`、`fabric` 与 `neoforge` 三大子模块，通过 Shadow 插件将 `common` 字节码及 MP3 音频解码库私有化阴影重定位（Relocate），最终由 Loom 输出包含 Mojang 正规映射表的发布包。

### 平台入口与事件挂载差异
- Fabric 端：通用入口 `MmdSkinFabric` 注册网络载荷，客户端入口 `MmdSkinFabricClient` 挂载 `FabricClientRuntimeHooks`，监听 ClientTick、进退服与 HudRender 事件；按键通过 `KeyBindingHelper` 注册；生物实体替换通过 Fabric API 的 `EntityRendererRegistry` 统一替换。
- NeoForge 端：主入口 `MmdSkinNeoForge` 区分 Mod 声明总线与运行时总线。Mod 总线监听按键注册与 `EntityRenderersEvent`；运行时总线通过 `NeoForgeClientEventHandler` 订阅 ClientTick、死亡事件与 `RenderGuiEvent.Post`。生物模型替换则注册在 `RenderLivingEvent.Pre`，并在接管后取消原版渲染。
- 旧版 Forge（1.20.1）：采用经典 EventBus 体系，客户端通过 `@EventBusSubscriber` 静态监听注册，作为向后移植与多版本兼容的代码参照基线。

### 平台特化机制与 Mixin 软兼容
- 网络传输：全面拥抱 1.21.1 规范，采用 `CustomPacketPayload` 与 `StreamCodec` 契约体系。Fabric 使用 `PayloadTypeRegistry` 与 `ServerPlayNetworking`，NeoForge 使用 `RegisterPayloadHandlersEvent` 与 `PacketDistributor`，业务端通过 `ClientNetworkBindings` 统一抽象分发。
- TaCZ 枪械兼容：在各端 Mixin 中使用 `@Pseudo` 伪装注入，捕获枪械在第一人称功能节点（包括开镜 ADS 状态与双手翻转）下的最终姿态矩阵，随后由 `TaczFirstPersonPostRenderer` 统一接管 MMD 双臂姿态并阻止原版手臂重复绘制，全程无需硬编译依赖。
- 原生库多平台装载：`NativeLibraryLoader` 在运行时精准嗅探 Windows、Linux、macOS 及 Android 移动容器（PojavLauncher 等），将动态库解压至带版本后缀的隔离目录并校验 SHA-256 指纹；载入后通过 JNI 核验版本号常量（`v1.10alpha`），防止因残留旧库引发 ABI 崩溃。

---

## 5. 核心开发接口与二次开发索引

为方便后续开发、调试以及外部扩展，模组暴露了清晰的编程接口与配置入口：

### 对外 API：com.shiroha.mmdskin.api.MmdSkinApi
- `getModelInfo(Player)`：安全获取指定玩家当前装配模型的骨骼总数、顶点数、材质分段及关键骨骼变换矩阵。
- `getUV(Player)`：读取实时的动态变形 UV 数据流。
- `setBoneOverride(Player, boneIndex, translation, rotation)`：允许外部程序化逻辑强行覆写指定骨骼的平移与四元数旋转，用于外部动作捕捉接入。
- `setExternalIkOverride(Player, boolean)`：接管或屏蔽模型的原生 IK 求解，允许外部算法主导肢体逆动力学。

### 本地文件与关键配置路径
- 客户端模型存储目录：`.minecraft/3d-skin/`（存放 PMX/VRM 模型文件夹及动作纹理）。
- 模组全局配置文件：`.minecraft/config/mmdskin.json`（图形渲染、物理仿真参数、性能预算）。
- 单模型独立配置：`.minecraft/3d-skin/<模型名>/config.json`（视线追踪范围、模型单独缩放比例、部件材质可见性开关）。
- 玩家独立替换配置：持久化于 `ModelSelectorConfig`，支持在游戏内配置界面或 Alt 快捷轮盘中按玩家名/UUID 指定独立展示模型。

---
*本索引文件由子代理全库代码扫描与架构分析自动化生成，作为工作区全模块认知与后续开发的唯一基准文档。*
