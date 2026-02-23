// Auto-generated NetMessage implementations

pub trait NetMessage: prost::Message {
    const CMD_ID: i32;
}

impl NetMessage for CsLogin {
    const CMD_ID: i32 = 1;
}

impl NetMessage for CsCreateRole {
    const CMD_ID: i32 = 2;
}

impl NetMessage for CsLogout {
    const CMD_ID: i32 = 3;
}

impl NetMessage for CsGmCommand {
    const CMD_ID: i32 = 4;
}

impl NetMessage for CsPing {
    const CMD_ID: i32 = 5;
}

impl NetMessage for CsFlushSync {
    const CMD_ID: i32 = 6;
}

impl NetMessage for CsAchieveComplete {
    const CMD_ID: i32 = 21;
}

impl NetMessage for CsAchieveTakeReward {
    const CMD_ID: i32 = 22;
}

impl NetMessage for CsCharBagSetTeam {
    const CMD_ID: i32 = 31;
}

impl NetMessage for CsCharBagSetCurrTeamIndex {
    const CMD_ID: i32 = 32;
}

impl NetMessage for CsCharBagSetTeamName {
    const CMD_ID: i32 = 33;
}

impl NetMessage for CsCharBagSetTeamLeader {
    const CMD_ID: i32 = 34;
}

impl NetMessage for CsCharLevelUp {
    const CMD_ID: i32 = 41;
}

impl NetMessage for CsCharBreak {
    const CMD_ID: i32 = 42;
}

impl NetMessage for CsCharSetNormalSkill {
    const CMD_ID: i32 = 43;
}

impl NetMessage for CsCharSetBattleInfo {
    const CMD_ID: i32 = 44;
}

impl NetMessage for CsCharSkillLevelUp {
    const CMD_ID: i32 = 45;
}

impl NetMessage for CsCharSetTeamSkill {
    const CMD_ID: i32 = 46;
}

impl NetMessage for CsEquipPuton {
    const CMD_ID: i32 = 51;
}

impl NetMessage for CsEquipPutoff {
    const CMD_ID: i32 = 52;
}

impl NetMessage for CsItemBagTidyInBag {
    const CMD_ID: i32 = 61;
}

impl NetMessage for CsItemBagMoveInBag {
    const CMD_ID: i32 = 62;
}

impl NetMessage for CsItemBagSplitInBag {
    const CMD_ID: i32 = 63;
}

impl NetMessage for CsItemBagFactoryDepotToBag {
    const CMD_ID: i32 = 64;
}

impl NetMessage for CsItemBagBagToFactoryDepot {
    const CMD_ID: i32 = 65;
}

impl NetMessage for CsItemBagDestroyInBag {
    const CMD_ID: i32 = 66;
}

impl NetMessage for CsItemBagDestroyInDepot {
    const CMD_ID: i32 = 67;
}

impl NetMessage for CsItemBagUseItem {
    const CMD_ID: i32 = 68;
}

impl NetMessage for CsItemBagFactoryDepotToBagGrid {
    const CMD_ID: i32 = 69;
}

impl NetMessage for CsItemBagSetQuickBar {
    const CMD_ID: i32 = 70;
}

impl NetMessage for CsItemBagSetQuickBarPos {
    const CMD_ID: i32 = 71;
}

impl NetMessage for CsItemBagSetItemLock {
    const CMD_ID: i32 = 72;
}

impl NetMessage for CsEnterScene {
    const CMD_ID: i32 = 81;
}

impl NetMessage for CsMoveObjectMove {
    const CMD_ID: i32 = 82;
}

impl NetMessage for CsSceneSetLastRecordCampid {
    const CMD_ID: i32 = 83;
}

impl NetMessage for CsSceneInteractiveEventTrigger {
    const CMD_ID: i32 = 84;
}

impl NetMessage for CsSceneSetVar {
    const CMD_ID: i32 = 85;
}

impl NetMessage for CsSceneRest {
    const CMD_ID: i32 = 86;
}

impl NetMessage for CsSceneKillMonster {
    const CMD_ID: i32 = 87;
}

impl NetMessage for CsSceneLoadFinish {
    const CMD_ID: i32 = 88;
}

impl NetMessage for CsSceneSpawnInteractive {
    const CMD_ID: i32 = 89;
}

impl NetMessage for CsSceneKillChar {
    const CMD_ID: i32 = 90;
}

impl NetMessage for CsSceneCreateEntity {
    const CMD_ID: i32 = 92;
}

impl NetMessage for CsSceneDestroyEntity {
    const CMD_ID: i32 = 93;
}

impl NetMessage for CsSceneUpdateInteractiveProperty {
    const CMD_ID: i32 = 94;
}

impl NetMessage for CsSceneSetSafeZone {
    const CMD_ID: i32 = 95;
}

impl NetMessage for CsSceneQueryEntityExist {
    const CMD_ID: i32 = 96;
}

impl NetMessage for CsSceneQueryInteractiveProperty {
    const CMD_ID: i32 = 97;
}

impl NetMessage for CsSceneSpawnMonster {
    const CMD_ID: i32 = 98;
}

impl NetMessage for CsSceneSetTrackPoint {
    const CMD_ID: i32 = 99;
}

impl NetMessage for CsSceneInteractTree {
    const CMD_ID: i32 = 100;
}

impl NetMessage for CsSceneMapMarkUpdateState {
    const CMD_ID: i32 = 101;
}

impl NetMessage for CsSceneMapMarkCreate {
    const CMD_ID: i32 = 102;
}

impl NetMessage for CsSceneTeleport {
    const CMD_ID: i32 = 103;
}

impl NetMessage for CsSceneMoveStateSet {
    const CMD_ID: i32 = 104;
}

impl NetMessage for CsSceneSubmitItem {
    const CMD_ID: i32 = 105;
}

impl NetMessage for CsSceneSubmitEther {
    const CMD_ID: i32 = 106;
}

impl NetMessage for CsSceneSetLevelScriptActive {
    const CMD_ID: i32 = 107;
}

impl NetMessage for CsSceneUpdateLevelScriptProperty {
    const CMD_ID: i32 = 108;
}

impl NetMessage for CsSceneLevelScriptEventTrigger {
    const CMD_ID: i32 = 109;
}

impl NetMessage for CsSceneCommitLevelScriptCacheStep {
    const CMD_ID: i32 = 110;
}

impl NetMessage for CsSceneResetEntity {
    const CMD_ID: i32 = 111;
}

impl NetMessage for CsSceneResetLevelScript {
    const CMD_ID: i32 = 112;
}

impl NetMessage for CsSceneSetRepatriatePoint {
    const CMD_ID: i32 = 114;
}

impl NetMessage for CsSceneRepatriate {
    const CMD_ID: i32 = 115;
}

impl NetMessage for CsSceneInteractSpInteractive {
    const CMD_ID: i32 = 116;
}

impl NetMessage for CsSceneSetCheckPoint {
    const CMD_ID: i32 = 117;
}

impl NetMessage for CsSceneSetBattle {
    const CMD_ID: i32 = 118;
}

impl NetMessage for CsSceneRevival {
    const CMD_ID: i32 = 119;
}

impl NetMessage for CsFactoryHsInout {
    const CMD_ID: i32 = 201;
}

impl NetMessage for CsFactoryHsFb {
    const CMD_ID: i32 = 202;
}

impl NetMessage for CsFactoryOp {
    const CMD_ID: i32 = 203;
}

impl NetMessage for CsFactoryQuickbarSetOne {
    const CMD_ID: i32 = 204;
}

impl NetMessage for CsFactoryQuickbarMoveOne {
    const CMD_ID: i32 = 205;
}

impl NetMessage for CsFactoryWorkshopMake {
    const CMD_ID: i32 = 210;
}

impl NetMessage for CsFactorySttUnlockNode {
    const CMD_ID: i32 = 211;
}

impl NetMessage for CsFactoryManuallyWorkAppend {
    const CMD_ID: i32 = 216;
}

impl NetMessage for CsFactoryManuallyWorkResume {
    const CMD_ID: i32 = 217;
}

impl NetMessage for CsFactoryManuallyWorkCancel {
    const CMD_ID: i32 = 218;
}

impl NetMessage for CsFactoryManuallyWorkPause {
    const CMD_ID: i32 = 219;
}

impl NetMessage for CsFactoryManufactureStart {
    const CMD_ID: i32 = 220;
}

impl NetMessage for CsFactoryManufactureSettle {
    const CMD_ID: i32 = 221;
}

impl NetMessage for CsFactoryTradeSetContract {
    const CMD_ID: i32 = 222;
}

impl NetMessage for CsFactoryTradeCashOrder {
    const CMD_ID: i32 = 223;
}

impl NetMessage for CsFactoryTradeDeleteOrder {
    const CMD_ID: i32 = 224;
}

impl NetMessage for CsFactoryRepairBuilding {
    const CMD_ID: i32 = 225;
}

impl NetMessage for CsFactoryManufactureCancel {
    const CMD_ID: i32 = 226;
}

impl NetMessage for CsFactoryRecyclerCommitMaterial {
    const CMD_ID: i32 = 227;
}

impl NetMessage for CsFactoryRecyclerFetchProduct {
    const CMD_ID: i32 = 228;
}

impl NetMessage for CsFactoryProcessorMarkUnlockFormulaRead {
    const CMD_ID: i32 = 229;
}

impl NetMessage for CsFactoryProcessorMakeItem {
    const CMD_ID: i32 = 230;
}

impl NetMessage for CsFactoryProcessorMakeEquip {
    const CMD_ID: i32 = 231;
}

impl NetMessage for CsFactoryProcessorMakeGem {
    const CMD_ID: i32 = 232;
}

impl NetMessage for CsFactoryProcessorRecastGem {
    const CMD_ID: i32 = 233;
}

impl NetMessage for CsFactoryCharacterWorkPunchIn {
    const CMD_ID: i32 = 234;
}

impl NetMessage for CsFactoryCharacterWorkPunchOut {
    const CMD_ID: i32 = 235;
}

impl NetMessage for CsFactoryStatisticSetBookmarkItemIds {
    const CMD_ID: i32 = 236;
}

impl NetMessage for CsFactoryStatisticRequire {
    const CMD_ID: i32 = 237;
}

impl NetMessage for CsFactorySoilPlant {
    const CMD_ID: i32 = 238;
}

impl NetMessage for CsFactorySoilCancel {
    const CMD_ID: i32 = 239;
}

impl NetMessage for CsFactorySoilHarvest {
    const CMD_ID: i32 = 240;
}

impl NetMessage for CsFactoryObserverOp {
    const CMD_ID: i32 = 258;
}

impl NetMessage for CsWeaponPuton {
    const CMD_ID: i32 = 271;
}

impl NetMessage for CsWeaponBreakthrough {
    const CMD_ID: i32 = 273;
}

impl NetMessage for CsWeaponAddExp {
    const CMD_ID: i32 = 274;
}

impl NetMessage for CsWeaponAttachGem {
    const CMD_ID: i32 = 275;
}

impl NetMessage for CsWeaponDetachGem {
    const CMD_ID: i32 = 276;
}

impl NetMessage for CsUnlockWiki {
    const CMD_ID: i32 = 291;
}

impl NetMessage for CsMarkWikiRead {
    const CMD_ID: i32 = 292;
}

impl NetMessage for CsWikiPin {
    const CMD_ID: i32 = 293;
}

impl NetMessage for CsFailMission {
    const CMD_ID: i32 = 311;
}

impl NetMessage for CsTrackMission {
    const CMD_ID: i32 = 313;
}

impl NetMessage for CsStopTrackingMission {
    const CMD_ID: i32 = 314;
}

impl NetMessage for CsUpdateQuestObjective {
    const CMD_ID: i32 = 315;
}

impl NetMessage for CsAcceptMission {
    const CMD_ID: i32 = 316;
}

impl NetMessage for CsRollBlocMission {
    const CMD_ID: i32 = 317;
}

impl NetMessage for CsCompleteGuideGroupKeyStep {
    const CMD_ID: i32 = 331;
}

impl NetMessage for CsCompleteGuideGroup {
    const CMD_ID: i32 = 332;
}

impl NetMessage for CsFinishDialog {
    const CMD_ID: i32 = 341;
}

impl NetMessage for CsBlocShopBuy {
    const CMD_ID: i32 = 352;
}

impl NetMessage for CsEnterDungeon {
    const CMD_ID: i32 = 371;
}

impl NetMessage for CsRestartDungeon {
    const CMD_ID: i32 = 372;
}

impl NetMessage for CsLeaveDungeon {
    const CMD_ID: i32 = 373;
}

impl NetMessage for CsDungeonReward {
    const CMD_ID: i32 = 374;
}

impl NetMessage for CsDungeonRecoverAp {
    const CMD_ID: i32 = 375;
}

impl NetMessage for CsGetMail {
    const CMD_ID: i32 = 401;
}

impl NetMessage for CsReadMail {
    const CMD_ID: i32 = 402;
}

impl NetMessage for CsDeleteMail {
    const CMD_ID: i32 = 403;
}

impl NetMessage for CsDeleteAllMail {
    const CMD_ID: i32 = 404;
}

impl NetMessage for CsGetMailAttachment {
    const CMD_ID: i32 = 405;
}

impl NetMessage for CsGetAllMailAttachment {
    const CMD_ID: i32 = 406;
}

impl NetMessage for CsRemoveItemNewTags {
    const CMD_ID: i32 = 431;
}

impl NetMessage for CsRedDotReadFormula {
    const CMD_ID: i32 = 432;
}

impl NetMessage for CsPrtsMarkRead {
    const CMD_ID: i32 = 442;
}

impl NetMessage for CsPrtsMarkTerminalRead {
    const CMD_ID: i32 = 443;
}

impl NetMessage for CsBitsetAdd {
    const CMD_ID: i32 = 481;
}

impl NetMessage for CsBitsetRemove {
    const CMD_ID: i32 = 482;
}

impl NetMessage for CsBitsetRemoveAll {
    const CMD_ID: i32 = 483;
}

impl NetMessage for CsMergeMsg {
    const CMD_ID: i32 = 500;
}

impl NetMessage for ScLogin {
    const CMD_ID: i32 = 1;
}

impl NetMessage for ScSyncBaseData {
    const CMD_ID: i32 = 2;
}

impl NetMessage for ScNtfErrorCode {
    const CMD_ID: i32 = 3;
}

impl NetMessage for ScGmCommand {
    const CMD_ID: i32 = 4;
}

impl NetMessage for ScPing {
    const CMD_ID: i32 = 5;
}

impl NetMessage for ScReconnectIncr {
    const CMD_ID: i32 = 6;
}

impl NetMessage for ScReconnectFull {
    const CMD_ID: i32 = 7;
}

impl NetMessage for ScFlushSync {
    const CMD_ID: i32 = 8;
}

impl NetMessage for ScNtfCode {
    const CMD_ID: i32 = 9;
}

impl NetMessage for ScAchieveComplete {
    const CMD_ID: i32 = 10;
}

impl NetMessage for ScSyncAllRoleScene {
    const CMD_ID: i32 = 20;
}

impl NetMessage for ScObjectEnterView {
    const CMD_ID: i32 = 21;
}

impl NetMessage for ScObjectLeaveView {
    const CMD_ID: i32 = 22;
}

impl NetMessage for ScMoveObjectMove {
    const CMD_ID: i32 = 23;
}

impl NetMessage for ScEnterSceneNotify {
    const CMD_ID: i32 = 24;
}

impl NetMessage for ScSelfSceneInfo {
    const CMD_ID: i32 = 25;
}

impl NetMessage for ScLeaveSceneNotify {
    const CMD_ID: i32 = 26;
}

impl NetMessage for ScSceneSetLastRecordCampid {
    const CMD_ID: i32 = 27;
}

impl NetMessage for ScSceneUpdateInteractiveProperty {
    const CMD_ID: i32 = 28;
}

impl NetMessage for ScSceneSetVar {
    const CMD_ID: i32 = 29;
}

impl NetMessage for ScSceneRevival {
    const CMD_ID: i32 = 30;
}

impl NetMessage for ScSceneCreateEntity {
    const CMD_ID: i32 = 31;
}

impl NetMessage for ScSceneDestroyEntity {
    const CMD_ID: i32 = 32;
}

impl NetMessage for ScSceneSetSafeZone {
    const CMD_ID: i32 = 35;
}

impl NetMessage for ScSceneQueryEntityExist {
    const CMD_ID: i32 = 36;
}

impl NetMessage for ScSceneLevelScriptStateNotify {
    const CMD_ID: i32 = 37;
}

impl NetMessage for ScSceneQueryInteractiveProperty {
    const CMD_ID: i32 = 38;
}

impl NetMessage for ScSceneUnlockArea {
    const CMD_ID: i32 = 39;
}

impl NetMessage for ScSceneSetTrackPoint {
    const CMD_ID: i32 = 40;
}

impl NetMessage for ScSceneCollectionSync {
    const CMD_ID: i32 = 42;
}

impl NetMessage for ScSceneCollectionModify {
    const CMD_ID: i32 = 43;
}

impl NetMessage for ScSceneMapMarkSync {
    const CMD_ID: i32 = 44;
}

impl NetMessage for ScSceneMapMarkModify {
    const CMD_ID: i32 = 45;
}

impl NetMessage for ScSceneTeleport {
    const CMD_ID: i32 = 46;
}

impl NetMessage for ScSceneSubmitItem {
    const CMD_ID: i32 = 47;
}

impl NetMessage for ScSceneSubmitEther {
    const CMD_ID: i32 = 48;
}

impl NetMessage for ScSceneUpdateLevelScriptProperty {
    const CMD_ID: i32 = 49;
}

impl NetMessage for ScSceneResetEntity {
    const CMD_ID: i32 = 50;
}

impl NetMessage for ScSceneLevelScriptResetBegin {
    const CMD_ID: i32 = 51;
}

impl NetMessage for ScSceneLevelScriptResetEnd {
    const CMD_ID: i32 = 52;
}

impl NetMessage for ScSceneSetBattle {
    const CMD_ID: i32 = 53;
}

impl NetMessage for ScSceneRevivalModeModify {
    const CMD_ID: i32 = 54;
}

impl NetMessage for ScSceneLevelScriptEventTrigger {
    const CMD_ID: i32 = 55;
}

impl NetMessage for ScSceneInteractiveEventTrigger {
    const CMD_ID: i32 = 56;
}

impl NetMessage for ScSyncCharBagInfo {
    const CMD_ID: i32 = 60;
}

impl NetMessage for ScCharBagAddChar {
    const CMD_ID: i32 = 61;
}

impl NetMessage for ScCharBagSetTeam {
    const CMD_ID: i32 = 62;
}

impl NetMessage for ScCharBagSetCurrTeamIndex {
    const CMD_ID: i32 = 63;
}

impl NetMessage for ScCharBagSetTeamName {
    const CMD_ID: i32 = 64;
}

impl NetMessage for ScCharBagSetTeamLeader {
    const CMD_ID: i32 = 65;
}

impl NetMessage for ScCharBagSetMaxTeamMemberCount {
    const CMD_ID: i32 = 66;
}

impl NetMessage for ScSyncWallet {
    const CMD_ID: i32 = 70;
}

impl NetMessage for ScWalletSyncMoney {
    const CMD_ID: i32 = 71;
}

impl NetMessage for ScCharLevelUp {
    const CMD_ID: i32 = 80;
}

impl NetMessage for ScCharBreak {
    const CMD_ID: i32 = 81;
}

impl NetMessage for ScCharSyncLevelExp {
    const CMD_ID: i32 = 82;
}

impl NetMessage for ScCharSetNormalSkill {
    const CMD_ID: i32 = 83;
}

impl NetMessage for ScCharSkillLevelUp {
    const CMD_ID: i32 = 84;
}

impl NetMessage for ScCharUnlockSkill {
    const CMD_ID: i32 = 85;
}

impl NetMessage for ScCharGainExpToast {
    const CMD_ID: i32 = 86;
}

impl NetMessage for ScCharSyncStatus {
    const CMD_ID: i32 = 87;
}

impl NetMessage for ScCharSetTeamSkill {
    const CMD_ID: i32 = 89;
}

impl NetMessage for ScEquipPuton {
    const CMD_ID: i32 = 90;
}

impl NetMessage for ScEquipPutoff {
    const CMD_ID: i32 = 91;
}

impl NetMessage for ScItemBagSync {
    const CMD_ID: i32 = 100;
}

impl NetMessage for ScItemBagSyncModify {
    const CMD_ID: i32 = 101;
}

impl NetMessage for ScItemBagUseItem {
    const CMD_ID: i32 = 102;
}

impl NetMessage for ScItemBagSyncQuickBar {
    const CMD_ID: i32 = 103;
}

impl NetMessage for ScItemBagSetQuickBar {
    const CMD_ID: i32 = 104;
}

impl NetMessage for ScItemBagSetQuickBarPos {
    const CMD_ID: i32 = 105;
}

impl NetMessage for ScItemBagSetItemLock {
    const CMD_ID: i32 = 106;
}

impl NetMessage for ScItemBagBagToFactoryDepot {
    const CMD_ID: i32 = 107;
}

impl NetMessage for ScSyncAllMission {
    const CMD_ID: i32 = 110;
}

impl NetMessage for ScQuestStateUpdate {
    const CMD_ID: i32 = 111;
}

impl NetMessage for ScMissionStateUpdate {
    const CMD_ID: i32 = 112;
}

impl NetMessage for ScQuestFailed {
    const CMD_ID: i32 = 113;
}

impl NetMessage for ScMissionFailed {
    const CMD_ID: i32 = 114;
}

impl NetMessage for ScTrackMissionChange {
    const CMD_ID: i32 = 115;
}

impl NetMessage for ScQuestObjectivesUpdate {
    const CMD_ID: i32 = 116;
}

impl NetMessage for ScMissionDeleted {
    const CMD_ID: i32 = 117;
}

impl NetMessage for ScRollBlocMission {
    const CMD_ID: i32 = 120;
}

impl NetMessage for ScSyncBlocMissionInfo {
    const CMD_ID: i32 = 121;
}

impl NetMessage for ScBlocCompletedMissionNumUpdate {
    const CMD_ID: i32 = 122;
}

impl NetMessage for ScSyncAllDialog {
    const CMD_ID: i32 = 130;
}

impl NetMessage for ScFinishDialog {
    const CMD_ID: i32 = 131;
}

impl NetMessage for ScSyncAllGuide {
    const CMD_ID: i32 = 140;
}

impl NetMessage for ScCompleteGuideGroupKeyStep {
    const CMD_ID: i32 = 141;
}

impl NetMessage for ScCompleteGuideGroup {
    const CMD_ID: i32 = 142;
}

impl NetMessage for ScAcceptGuideGroup {
    const CMD_ID: i32 = 143;
}

impl NetMessage for ScSyncAttr {
    const CMD_ID: i32 = 150;
}

impl NetMessage for ScSyncAllUnlock {
    const CMD_ID: i32 = 160;
}

impl NetMessage for ScUnlockSystem {
    const CMD_ID: i32 = 161;
}

impl NetMessage for ScSyncAllBitset {
    const CMD_ID: i32 = 165;
}

impl NetMessage for ScBitsetAdd {
    const CMD_ID: i32 = 166;
}

impl NetMessage for ScBitsetRemove {
    const CMD_ID: i32 = 167;
}

impl NetMessage for ScBitsetRemoveAll {
    const CMD_ID: i32 = 168;
}

impl NetMessage for ScFactorySync {
    const CMD_ID: i32 = 200;
}

impl NetMessage for ScFactorySyncContext {
    const CMD_ID: i32 = 201;
}

impl NetMessage for ScFactoryModify {
    const CMD_ID: i32 = 202;
}

impl NetMessage for ScFactoryNotify {
    const CMD_ID: i32 = 203;
}

impl NetMessage for ScFactoryModifyStt {
    const CMD_ID: i32 = 204;
}

impl NetMessage for ScFactoryModifyFormulaMan {
    const CMD_ID: i32 = 205;
}

impl NetMessage for ScFactoryModifyManuallyWork {
    const CMD_ID: i32 = 206;
}

impl NetMessage for ScFactoryModifyManufacture {
    const CMD_ID: i32 = 207;
}

impl NetMessage for ScFactoryModifyTrade {
    const CMD_ID: i32 = 208;
}

impl NetMessage for ScFactoryModifyRepair {
    const CMD_ID: i32 = 209;
}

impl NetMessage for ScFactoryModifyWorkshop {
    const CMD_ID: i32 = 210;
}

impl NetMessage for ScFactoryModifyRecycler {
    const CMD_ID: i32 = 211;
}

impl NetMessage for ScFactoryModifyContext {
    const CMD_ID: i32 = 212;
}

impl NetMessage for ScFactoryModifyRegionNodes {
    const CMD_ID: i32 = 213;
}

impl NetMessage for ScFactoryModifyRegionComponents {
    const CMD_ID: i32 = 214;
}

impl NetMessage for ScFactoryModifyRegionScene {
    const CMD_ID: i32 = 215;
}

impl NetMessage for ScFactoryModifyQuickbar {
    const CMD_ID: i32 = 216;
}

impl NetMessage for ScFactoryModifyProcessor {
    const CMD_ID: i32 = 217;
}

impl NetMessage for ScFactoryModifyCharacterWork {
    const CMD_ID: i32 = 218;
}

impl NetMessage for ScFactorySyncRegionNodes {
    const CMD_ID: i32 = 219;
}

impl NetMessage for ScFactorySyncStatistic {
    const CMD_ID: i32 = 220;
}

impl NetMessage for ScFactoryHs {
    const CMD_ID: i32 = 221;
}

impl NetMessage for ScFactoryHsSync {
    const CMD_ID: i32 = 222;
}

impl NetMessage for ScFactoryOpRet {
    const CMD_ID: i32 = 223;
}

impl NetMessage for ScFactoryCommonRet {
    const CMD_ID: i32 = 230;
}

impl NetMessage for ScFactoryManuallyWorkCancel {
    const CMD_ID: i32 = 231;
}

impl NetMessage for ScFactoryManufactureStart {
    const CMD_ID: i32 = 232;
}

impl NetMessage for ScFactoryManufactureSettle {
    const CMD_ID: i32 = 233;
}

impl NetMessage for ScFactoryProcessorRet {
    const CMD_ID: i32 = 234;
}

impl NetMessage for ScFactoryTradeCashOrder {
    const CMD_ID: i32 = 235;
}

impl NetMessage for ScFactorySyncCharacterWork {
    const CMD_ID: i32 = 236;
}

impl NetMessage for ScFactoryManufactureCancel {
    const CMD_ID: i32 = 237;
}

impl NetMessage for ScFactoryRecyclerFetchProduct {
    const CMD_ID: i32 = 238;
}

impl NetMessage for ScFactoryRecyclerCommitMaterial {
    const CMD_ID: i32 = 239;
}

impl NetMessage for ScFactorySyncSkillBoard {
    const CMD_ID: i32 = 240;
}

impl NetMessage for ScFactoryModifySkillBoard {
    const CMD_ID: i32 = 241;
}

impl NetMessage for ScFactoryModifyStatistic {
    const CMD_ID: i32 = 242;
}

impl NetMessage for ScFactoryStatisticRequire {
    const CMD_ID: i32 = 243;
}

impl NetMessage for ScFactoryModifySoil {
    const CMD_ID: i32 = 244;
}

impl NetMessage for ScFactorySoilPlant {
    const CMD_ID: i32 = 245;
}

impl NetMessage for ScFactorySoilCancel {
    const CMD_ID: i32 = 246;
}

impl NetMessage for ScFactorySoilHarvest {
    const CMD_ID: i32 = 247;
}

impl NetMessage for ScFactoryModifyVisibleFormula {
    const CMD_ID: i32 = 248;
}

impl NetMessage for ScFactoryObserverRet {
    const CMD_ID: i32 = 249;
}

impl NetMessage for ScWeaponPuton {
    const CMD_ID: i32 = 250;
}

impl NetMessage for ScWeaponBreakthrough {
    const CMD_ID: i32 = 252;
}

impl NetMessage for ScWeaponAddExp {
    const CMD_ID: i32 = 253;
}

impl NetMessage for ScWeaponAttachGem {
    const CMD_ID: i32 = 254;
}

impl NetMessage for ScWeaponDetachGem {
    const CMD_ID: i32 = 255;
}

impl NetMessage for ScRewardToastBegin {
    const CMD_ID: i32 = 260;
}

impl NetMessage for ScRewardToastEnd {
    const CMD_ID: i32 = 261;
}

impl NetMessage for ScRewardDropMoneyToast {
    const CMD_ID: i32 = 262;
}

impl NetMessage for ScRewardToSceneBegin {
    const CMD_ID: i32 = 263;
}

impl NetMessage for ScRewardToSceneEnd {
    const CMD_ID: i32 = 264;
}

impl NetMessage for ScSyncAllBloc {
    const CMD_ID: i32 = 270;
}

impl NetMessage for ScBlocSyncLevel {
    const CMD_ID: i32 = 271;
}

impl NetMessage for ScBlocShopBuy {
    const CMD_ID: i32 = 273;
}

impl NetMessage for ScEnterDungeon {
    const CMD_ID: i32 = 301;
}

impl NetMessage for ScRestartDungeon {
    const CMD_ID: i32 = 302;
}

impl NetMessage for ScLeaveDungeon {
    const CMD_ID: i32 = 303;
}

impl NetMessage for ScSyncStamina {
    const CMD_ID: i32 = 304;
}

impl NetMessage for ScSyncDungeonPassStatus {
    const CMD_ID: i32 = 305;
}

impl NetMessage for ScSyncFullDungeonStatus {
    const CMD_ID: i32 = 306;
}

impl NetMessage for ScStartDungeonChallenge {
    const CMD_ID: i32 = 307;
}

impl NetMessage for ScDungeonReward {
    const CMD_ID: i32 = 308;
}

impl NetMessage for ScSyncDungeonChallengeStatus {
    const CMD_ID: i32 = 309;
}

impl NetMessage for ScSyncAllMail {
    const CMD_ID: i32 = 400;
}

impl NetMessage for ScGetMail {
    const CMD_ID: i32 = 401;
}

impl NetMessage for ScReadMail {
    const CMD_ID: i32 = 402;
}

impl NetMessage for ScGetMailAttachment {
    const CMD_ID: i32 = 403;
}

impl NetMessage for ScDelMailNotify {
    const CMD_ID: i32 = 404;
}

impl NetMessage for ScNewMailNotify {
    const CMD_ID: i32 = 405;
}

impl NetMessage for ScSyncExtraAttachmentItem {
    const CMD_ID: i32 = 406;
}

impl NetMessage for ScSyncGameMode {
    const CMD_ID: i32 = 430;
}

impl NetMessage for ScRemoveItemNewTags {
    const CMD_ID: i32 = 440;
}

impl NetMessage for ScSyncWikiPin {
    const CMD_ID: i32 = 450;
}

impl NetMessage for ScSyncStatistic {
    const CMD_ID: i32 = 500;
}

impl NetMessage for ScNewNoticeNotify {
    const CMD_ID: i32 = 600;
}

