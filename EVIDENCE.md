# 协议与验证证据

参考资料根：`decompile/api-enhanced-main`。该目录只读；没有启动 Node、运行项目脚本、安装依赖或读取用户凭据/系统 token 缓存。

| 功能 | 确认的远端路径 / crypto | 源码依据（相对资料根） |
|---|---|---|
| xeapi 公钥 | `https://interface.music.163.com/api/bsr/sk/get`，普通 POST + HMAC 签名，AES-256-ECB 公钥密文 | `module/register_xeapikey.js:19-63`、`util/xeapiKey.js` |
| 游客注册 | `/api/register/anonimous`，xeapi；52 位大写十六进制设备、XOR/MD5/base64 username | `module/register_anonimous.js:15-42`、`util/index.js:175-182` |
| 二维码 | `/api/login/qrcode/unikey`、`/api/login/qrcode/client/login`，默认 eapi，type=3 | `module/login_qr_key.js`、`module/login_qr_check.js`；二维码内容 `module/login_qr_create.js:10` |
| 账号 | `/api/nuser/account/get`，weapi | `module/user_account.js` |
| 搜索 | `/api/cloudsearch/pc`，默认 eapi；s/type=1/limit/offset/total=true | `module/cloudsearch.js` |
| 歌单目录 | `/api/user/playlist`，weapi；uid/limit/offset/includeVideo | `module/user_playlist.js` |
| 歌单完整分页 | `/api/v6/playlist/detail` 默认 eapi，n=100000；对 trackIds 分页后 `/api/v3/song/detail`，weapi、c 为 JSON 数组字符串 | `module/playlist_detail.js`、`module/playlist_track_all.js:15-28`、`module/song_detail.js` |
| 本人推荐 | `/api/v1/discovery/recommend/resource`、`/api/v3/discovery/recommend/songs`，weapi | `module/recommend_resource.js`、`module/recommend_songs.js` |
| 公开推荐 | `/api/personalized/playlist`、`/api/personalized/newsong`，weapi | `module/personalized.js`、`module/personalized_newsong.js` |
| 播放 | `/api/song/enhance/player/url/v1`，xeapi；ids 字符串数组、level、encodeType=flac | `module/song_url_v1.js:15-20,68`；没有复制其 unblock 分支或任何 server 拦截器 |
| 歌词 | `/api/song/lyric/v1`，默认 eapi；id/cp/tv/lv/rv/kv/yv/ytv/yrv | `module/lyric_new.js`；YRC 格式 `public/docs/home.md:1549-1585` |

域名及默认 eapi/非加密响应配置：`util/config.json:13-23`。完整 header/path/response 行为：`util/request.js:269-339,382-441,558-578`。crypto/key 常量和 xeapi封装：`util/crypto.js:6-20,65-119,184-193,224-318`。官方风控拒绝不会改写 IP 或伪造权限。

MIT 许可证实际核对：资料根 `LICENSE` 为 `Copyright (c) 2013-2022 Binaryify`，原文保留于 `LICENSE-NETEASE`。非官方逆向协议可能随网易云升级变化；代码许可证不等同于平台服务授权或音乐版权授权。

## 2026-10-09 受控公开验证

- 新随机设备，从官方公钥接口得到并验证签名及解密公钥；官方 xeapi 注册取得全新 MUSIC_A，没有载入既有 Cookie。
- 游客注册有间歇性官方安全验证错误，真实返回失败；没有重写 IP/权益或跨平台替代。两次受控复验成功，后续强制新游客复验再次失败，不能承诺上游注册始终可用。
- 搜索“贝多芬”：返回 30 条真实歌曲；公开 weapi 推荐、歌词和标准播放接口成功。
- 只选搜索结果 `fee == 0` 的公开歌曲；player-url `code=200`、`level=standard`、`type=mp3`、`freeTrialInfo=null`；播放域名通过网易云 `*.music.126.net` 校验。
- 音频最小范围请求 `bytes=0-262143`，Content-Type `audio/mpeg; charset=UTF-8`；Symphonia 实际解码 **1152 音频帧**。没有打印或持久化签名 URL。
- 默认离线测试真实装载 `netease.dll` 到 AppContainer，完成 ABI1、所有能力握手、FileApi/DPAPI 设备持久化、注销后恢复未登录及版本拒绝测试。
- 扩展后的公开歌单完整 ID/详情批次、新歌推荐在线验证因后续游客注册风控尚未执行成功；相关完整分页/新歌解析已有合成 loopback 测试。默认测试 **31 个通过，1 个公开网络测试默认忽略**；clippy `--all-targets -- -D warnings` 无警告。

合成测试覆盖 eapi/weapi 固定向量、xeapi 公钥签名/X25519/GCM/AES/32字节会话密钥复用/gzip、真实 loopback 请求、123 首歌单三页、205 个目录歌单三页、歌手及毫秒时长、登录状态/验证后原子保存/失败回滚/跨实例注销、YRC/LRC/译文及试听/权限/降级/域名拒绝。

未经真实本人在线验证的功能：扫码授权、账号/个人歌单、会员音质。对应实现有合成测试，不能据此声称任何账号一定有播放权益。

## 主代理真实 DLL 集成验收

- Release `netease.dll` 经现有 `dist/pluginloader.exe` 在全新临时 runtime/AppContainer 中加载，没有复制实际安装目录的账号或游客凭据。
- `source-catalog --public-browse netease` 通过：游客公开推荐30个歌单、30首新歌，首个歌单首批50首歌曲详情，搜索50首歌曲。结果明确标记公开，未请求账号或音频。
- `source-playback` 搜索“贝多芬”，只选第一首服务端实际返回的歌曲，经插件 `Resolve` 标准音质及非试听/域名检查后，由宿主正常缓存654986字节音频，再交同一 Symphonia/CPAL 链静音解码。真实时钟前进，暂停冻结，2秒跳转均通过；探测时长约40.908秒。不输出曲目定位符、Cookie或签名URL，结果只证明该次所选资源通过非试听检查并能正常解码、暂停和跳转，不代表已经播放到整首结束。
- 主程序269项回归已通过，覆盖新增网易云不改变已保存的QQ/酷狗选择、三源独立音质偏好和正常的播放源切换。既有主程序及QQ/酷狗DLL不需替换。
- 上述验证补齐了此前被游客注册风控阻断的公开新歌和歌单首批真实解析；长歌单所有页、手机扫码、私人库及会员音质仍只有合成测试覆盖，不据一次成功承诺上游长期稳定或全部权益。

## 封面 HTTP 元数据修复

- `public/docs/home.md:256` 给出的官方封面示例采用 HTTP，支持图片尺寸 query；播放器图片加载器只允许 HTTPS。原网易云插件却直接丢弃 HTTP 元数据，造成合法封面缺失，而不是音频播放权限失败。
- 全新游客公开搜索实测30首歌均提供 HTTP 专辑封面；修复前通用 Track 中封面数为0，修复后为30。升级后 HTTPS 图片正常返回，限长/尺寸受限解码得到800×800图片，66902字节。
- 仅对合法 `music.126.net` 及其子域的 HTTP(S) 图片升级 HTTPS，保留原路径和尺寸 query；仍拒绝域名伪装、凭据、自定义端口、非Web协议。歌单封面和账号头像使用同一校验函数，不改账号、音频URL或权限流程。
- 新增合成回归覆盖歌曲/歌单/头像转换及域名安全；默认离线测试33项通过，2项公开网络测试保持默认忽略。封面专项公开测试显式执行通过，不读取已有Cookie或登录记录。
- 修复后的真实 Release DLL/AppContainer 公开检查也通过：30首推荐歌曲、30个推荐歌单、歌单首批50首歌曲和50首搜索结果均带合法封面地址，未请求账号或音频。安装只更新网易云DLL及说明文档，旧文件备份，未改播放器设置或登录数据。

## 2026-10-10 API 2 播放白名单遗漏修复

- `media::publicUrl` 与现有协议测试明确接受官方 `music.126.net` 及其子域的HTTP(S)音频，但API2首次部署的manifest只声明HTTPS。宿主匹配包含协议，导致插件合法解析的HTTP音频在结果检查和媒体加载阶段被拒绝为“网络请求失败：目标未声明或请求被拒绝”。
- 新回归使用打包的真实plugin.json，经插件的播放权限/非试听/精确音质解析，再交宿主NetworkPolicy；修复前在HTTP歌曲上失败，修复后根域/两个音频子域、两种协议和全部五档音质均通过，原路径及签名query不变。
- 仅增加 `http://*.music.126.net` 和 `http://music.126.net`，不改通用匹配规则、插件网络隔离、重定向规则或服务端版权/账号权限。HTTP账号接口、伪装域、凭据、自定义端口、本地与其它平台地址仍拒绝。
- 默认网易云35项离线测试通过（2项公开测试仍默认忽略），含真实DLL/AppContainer握手。使用安装DLL及修复manifest做全新游客静音播放复验，网易云返回官方安全验证400，在取得播放URL前失败；未读取既有账号/游客Cookie、未绕过风控，也不声称该次线上解码成功。
- 已原子更新 `dist/plugins/netease/plugin.json` 并备份原文件；保留既有options和其它网络声明。安装版再次通过隔离身份/能力检查，主程序、加载器及三个DLL的SHA-256均未改变，无需重编译或重新登录。现有运行实例须重启才采用新策略。
