// GENERATED CODE -- DO NOT EDIT!

'use strict';
var grpc = require('@grpc/grpc-js');
var management_interface_pb = require('./management_interface_pb.js');
var google_protobuf_empty_pb = require('google-protobuf/google/protobuf/empty_pb.js');
var google_protobuf_timestamp_pb = require('google-protobuf/google/protobuf/timestamp_pb.js');
var google_protobuf_wrappers_pb = require('google-protobuf/google/protobuf/wrappers_pb.js');
var google_protobuf_duration_pb = require('google-protobuf/google/protobuf/duration_pb.js');

function serialize_google_protobuf_BoolValue(arg) {
  if (!(arg instanceof google_protobuf_wrappers_pb.BoolValue)) {
    throw new Error('Expected argument of type google.protobuf.BoolValue');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_google_protobuf_BoolValue(buffer_arg) {
  return google_protobuf_wrappers_pb.BoolValue.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_google_protobuf_Duration(arg) {
  if (!(arg instanceof google_protobuf_duration_pb.Duration)) {
    throw new Error('Expected argument of type google.protobuf.Duration');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_google_protobuf_Duration(buffer_arg) {
  return google_protobuf_duration_pb.Duration.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_google_protobuf_Empty(arg) {
  if (!(arg instanceof google_protobuf_empty_pb.Empty)) {
    throw new Error('Expected argument of type google.protobuf.Empty');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_google_protobuf_Empty(buffer_arg) {
  return google_protobuf_empty_pb.Empty.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_google_protobuf_Int32Value(arg) {
  if (!(arg instanceof google_protobuf_wrappers_pb.Int32Value)) {
    throw new Error('Expected argument of type google.protobuf.Int32Value');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_google_protobuf_Int32Value(buffer_arg) {
  return google_protobuf_wrappers_pb.Int32Value.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_google_protobuf_StringValue(arg) {
  if (!(arg instanceof google_protobuf_wrappers_pb.StringValue)) {
    throw new Error('Expected argument of type google.protobuf.StringValue');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_google_protobuf_StringValue(buffer_arg) {
  return google_protobuf_wrappers_pb.StringValue.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_google_protobuf_UInt32Value(arg) {
  if (!(arg instanceof google_protobuf_wrappers_pb.UInt32Value)) {
    throw new Error('Expected argument of type google.protobuf.UInt32Value');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_google_protobuf_UInt32Value(buffer_arg) {
  return google_protobuf_wrappers_pb.UInt32Value.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_AccessMethodSetting(arg) {
  if (!(arg instanceof management_interface_pb.AccessMethodSetting)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.AccessMethodSetting');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_AccessMethodSetting(buffer_arg) {
  return management_interface_pb.AccessMethodSetting.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_AccountData(arg) {
  if (!(arg instanceof management_interface_pb.AccountData)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.AccountData');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_AccountData(buffer_arg) {
  return management_interface_pb.AccountData.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_AccountHistory(arg) {
  if (!(arg instanceof management_interface_pb.AccountHistory)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.AccountHistory');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_AccountHistory(buffer_arg) {
  return management_interface_pb.AccountHistory.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_AllowedIpsList(arg) {
  if (!(arg instanceof management_interface_pb.AllowedIpsList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.AllowedIpsList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_AllowedIpsList(buffer_arg) {
  return management_interface_pb.AllowedIpsList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_AppUpgradeEvent(arg) {
  if (!(arg instanceof management_interface_pb.AppUpgradeEvent)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.AppUpgradeEvent');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_AppUpgradeEvent(buffer_arg) {
  return management_interface_pb.AppUpgradeEvent.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_AppVersionInfo(arg) {
  if (!(arg instanceof management_interface_pb.AppVersionInfo)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.AppVersionInfo');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_AppVersionInfo(buffer_arg) {
  return management_interface_pb.AppVersionInfo.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_BridgeList(arg) {
  if (!(arg instanceof management_interface_pb.BridgeList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.BridgeList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_BridgeList(buffer_arg) {
  return management_interface_pb.BridgeList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_CustomList(arg) {
  if (!(arg instanceof management_interface_pb.CustomList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.CustomList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_CustomList(buffer_arg) {
  return management_interface_pb.CustomList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_CustomProxy(arg) {
  if (!(arg instanceof management_interface_pb.CustomProxy)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.CustomProxy');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_CustomProxy(buffer_arg) {
  return management_interface_pb.CustomProxy.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_DaemonEvent(arg) {
  if (!(arg instanceof management_interface_pb.DaemonEvent)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.DaemonEvent');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_DaemonEvent(buffer_arg) {
  return management_interface_pb.DaemonEvent.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_DaitaSettings(arg) {
  if (!(arg instanceof management_interface_pb.DaitaSettings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.DaitaSettings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_DaitaSettings(buffer_arg) {
  return management_interface_pb.DaitaSettings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_DeviceList(arg) {
  if (!(arg instanceof management_interface_pb.DeviceList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.DeviceList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_DeviceList(buffer_arg) {
  return management_interface_pb.DeviceList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_DeviceRemoval(arg) {
  if (!(arg instanceof management_interface_pb.DeviceRemoval)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.DeviceRemoval');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_DeviceRemoval(buffer_arg) {
  return management_interface_pb.DeviceRemoval.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_DeviceState(arg) {
  if (!(arg instanceof management_interface_pb.DeviceState)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.DeviceState');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_DeviceState(buffer_arg) {
  return management_interface_pb.DeviceState.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_DnsOptions(arg) {
  if (!(arg instanceof management_interface_pb.DnsOptions)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.DnsOptions');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_DnsOptions(buffer_arg) {
  return management_interface_pb.DnsOptions.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ExcludedProcessList(arg) {
  if (!(arg instanceof management_interface_pb.ExcludedProcessList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ExcludedProcessList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ExcludedProcessList(buffer_arg) {
  return management_interface_pb.ExcludedProcessList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_FeatureIndicators(arg) {
  if (!(arg instanceof management_interface_pb.FeatureIndicators)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.FeatureIndicators');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_FeatureIndicators(buffer_arg) {
  return management_interface_pb.FeatureIndicators.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ForumAttachLogsRequest(arg) {
  if (!(arg instanceof management_interface_pb.ForumAttachLogsRequest)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ForumAttachLogsRequest');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ForumAttachLogsRequest(buffer_arg) {
  return management_interface_pb.ForumAttachLogsRequest.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ForumLoginRequest(arg) {
  if (!(arg instanceof management_interface_pb.ForumLoginRequest)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ForumLoginRequest');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ForumLoginRequest(buffer_arg) {
  return management_interface_pb.ForumLoginRequest.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ForumLoginSignature(arg) {
  if (!(arg instanceof management_interface_pb.ForumLoginSignature)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ForumLoginSignature');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ForumLoginSignature(buffer_arg) {
  return management_interface_pb.ForumLoginSignature.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_LogFilter(arg) {
  if (!(arg instanceof management_interface_pb.LogFilter)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.LogFilter');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_LogFilter(buffer_arg) {
  return management_interface_pb.LogFilter.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_LogMessage(arg) {
  if (!(arg instanceof management_interface_pb.LogMessage)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.LogMessage');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_LogMessage(buffer_arg) {
  return management_interface_pb.LogMessage.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_NatPmpSettings(arg) {
  if (!(arg instanceof management_interface_pb.NatPmpSettings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.NatPmpSettings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_NatPmpSettings(buffer_arg) {
  return management_interface_pb.NatPmpSettings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_NatPmpStatus(arg) {
  if (!(arg instanceof management_interface_pb.NatPmpStatus)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.NatPmpStatus');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_NatPmpStatus(buffer_arg) {
  return management_interface_pb.NatPmpStatus.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_NewAccessMethodSetting(arg) {
  if (!(arg instanceof management_interface_pb.NewAccessMethodSetting)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.NewAccessMethodSetting');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_NewAccessMethodSetting(buffer_arg) {
  return management_interface_pb.NewAccessMethodSetting.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_NewCustomList(arg) {
  if (!(arg instanceof management_interface_pb.NewCustomList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.NewCustomList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_NewCustomList(buffer_arg) {
  return management_interface_pb.NewCustomList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ObfuscationSettings(arg) {
  if (!(arg instanceof management_interface_pb.ObfuscationSettings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ObfuscationSettings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ObfuscationSettings(buffer_arg) {
  return management_interface_pb.ObfuscationSettings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_PlayExternalObfuscatedAccountId(arg) {
  if (!(arg instanceof management_interface_pb.PlayExternalObfuscatedAccountId)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.PlayExternalObfuscatedAccountId');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_PlayExternalObfuscatedAccountId(buffer_arg) {
  return management_interface_pb.PlayExternalObfuscatedAccountId.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_PlayPurchase(arg) {
  if (!(arg instanceof management_interface_pb.PlayPurchase)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.PlayPurchase');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_PlayPurchase(buffer_arg) {
  return management_interface_pb.PlayPurchase.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_PublicKey(arg) {
  if (!(arg instanceof management_interface_pb.PublicKey)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.PublicKey');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_PublicKey(buffer_arg) {
  return management_interface_pb.PublicKey.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_QuantumResistantState(arg) {
  if (!(arg instanceof management_interface_pb.QuantumResistantState)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.QuantumResistantState');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_QuantumResistantState(buffer_arg) {
  return management_interface_pb.QuantumResistantState.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_RelayList(arg) {
  if (!(arg instanceof management_interface_pb.RelayList)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.RelayList');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_RelayList(buffer_arg) {
  return management_interface_pb.RelayList.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_RelayOverride(arg) {
  if (!(arg instanceof management_interface_pb.RelayOverride)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.RelayOverride');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_RelayOverride(buffer_arg) {
  return management_interface_pb.RelayOverride.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_RelaySettings(arg) {
  if (!(arg instanceof management_interface_pb.RelaySettings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.RelaySettings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_RelaySettings(buffer_arg) {
  return management_interface_pb.RelaySettings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ReportPubkeyMismatchRequest(arg) {
  if (!(arg instanceof management_interface_pb.ReportPubkeyMismatchRequest)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ReportPubkeyMismatchRequest');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ReportPubkeyMismatchRequest(buffer_arg) {
  return management_interface_pb.ReportPubkeyMismatchRequest.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_ResetPinnedExitKeysResponse(arg) {
  if (!(arg instanceof management_interface_pb.ResetPinnedExitKeysResponse)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.ResetPinnedExitKeysResponse');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_ResetPinnedExitKeysResponse(buffer_arg) {
  return management_interface_pb.ResetPinnedExitKeysResponse.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_Rollout(arg) {
  if (!(arg instanceof management_interface_pb.Rollout)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.Rollout');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_Rollout(buffer_arg) {
  return management_interface_pb.Rollout.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_Seed(arg) {
  if (!(arg instanceof management_interface_pb.Seed)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.Seed');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_Seed(buffer_arg) {
  return management_interface_pb.Seed.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_Settings(arg) {
  if (!(arg instanceof management_interface_pb.Settings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.Settings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_Settings(buffer_arg) {
  return management_interface_pb.Settings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_SplitFilterMigration(arg) {
  if (!(arg instanceof management_interface_pb.SplitFilterMigration)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.SplitFilterMigration');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_SplitFilterMigration(buffer_arg) {
  return management_interface_pb.SplitFilterMigration.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_TrustNewExitKeyRequest(arg) {
  if (!(arg instanceof management_interface_pb.TrustNewExitKeyRequest)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.TrustNewExitKeyRequest');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_TrustNewExitKeyRequest(buffer_arg) {
  return management_interface_pb.TrustNewExitKeyRequest.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_TrustNewExitKeyResponse(arg) {
  if (!(arg instanceof management_interface_pb.TrustNewExitKeyResponse)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.TrustNewExitKeyResponse');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_TrustNewExitKeyResponse(buffer_arg) {
  return management_interface_pb.TrustNewExitKeyResponse.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_TunnelState(arg) {
  if (!(arg instanceof management_interface_pb.TunnelState)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.TunnelState');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_TunnelState(buffer_arg) {
  return management_interface_pb.TunnelState.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_UUID(arg) {
  if (!(arg instanceof management_interface_pb.UUID)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.UUID');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_UUID(buffer_arg) {
  return management_interface_pb.UUID.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_VoucherSubmission(arg) {
  if (!(arg instanceof management_interface_pb.VoucherSubmission)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.VoucherSubmission');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_VoucherSubmission(buffer_arg) {
  return management_interface_pb.VoucherSubmission.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_WarrenCustomExitSettings(arg) {
  if (!(arg instanceof management_interface_pb.WarrenCustomExitSettings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.WarrenCustomExitSettings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_WarrenCustomExitSettings(buffer_arg) {
  return management_interface_pb.WarrenCustomExitSettings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_WarrenMultiHopSettings(arg) {
  if (!(arg instanceof management_interface_pb.WarrenMultiHopSettings)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.WarrenMultiHopSettings');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_WarrenMultiHopSettings(buffer_arg) {
  return management_interface_pb.WarrenMultiHopSettings.deserializeBinary(new Uint8Array(buffer_arg));
}

function serialize_mullvad_daemon_management_interface_WarrenStatus(arg) {
  if (!(arg instanceof management_interface_pb.WarrenStatus)) {
    throw new Error('Expected argument of type mullvad_daemon.management_interface.WarrenStatus');
  }
  return Buffer.from(arg.serializeBinary());
}

function deserialize_mullvad_daemon_management_interface_WarrenStatus(buffer_arg) {
  return management_interface_pb.WarrenStatus.deserializeBinary(new Uint8Array(buffer_arg));
}


var ManagementServiceService = exports.ManagementServiceService = {
  // Control and get tunnel state
connectTunnel: {
    path: '/mullvad_daemon.management_interface.ManagementService/ConnectTunnel',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  disconnectTunnel: {
    path: '/mullvad_daemon.management_interface.ManagementService/DisconnectTunnel',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  reconnectTunnel: {
    path: '/mullvad_daemon.management_interface.ManagementService/ReconnectTunnel',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  getTunnelState: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetTunnelState',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.TunnelState,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_TunnelState,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_TunnelState,
  },
  // Control the daemon and receive events
eventsListen: {
    path: '/mullvad_daemon.management_interface.ManagementService/EventsListen',
    requestStream: false,
    responseStream: true,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.DaemonEvent,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_DaemonEvent,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_DaemonEvent,
  },
  // DEPRECATED: Prefer PrepareRestartV2.
prepareRestart: {
    path: '/mullvad_daemon.management_interface.ManagementService/PrepareRestart',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Takes a a boolean argument which says whether the daemon should stop after
// it is done preparing for a restart.
prepareRestartV2: {
    path: '/mullvad_daemon.management_interface.ManagementService/PrepareRestartV2',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  factoryReset: {
    path: '/mullvad_daemon.management_interface.ManagementService/FactoryReset',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getCurrentVersion: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetCurrentVersion',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  // Get information about the latest available version of the app.
// Note that calling this during an in-app upgrade will cancel the upgrade.
getVersionInfo: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetVersionInfo',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.AppVersionInfo,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_AppVersionInfo,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_AppVersionInfo,
  },
  isPerformingPostUpgrade: {
    path: '/mullvad_daemon.management_interface.ManagementService/IsPerformingPostUpgrade',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  // Relays and tunnel constraints
updateRelayLocations: {
    path: '/mullvad_daemon.management_interface.ManagementService/UpdateRelayLocations',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getRelayLocations: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetRelayLocations',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.RelayList,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_RelayList,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_RelayList,
  },
  setRelaySettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetRelaySettings',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.RelaySettings,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_RelaySettings,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_RelaySettings,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setObfuscationSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetObfuscationSettings',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.ObfuscationSettings,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_ObfuscationSettings,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_ObfuscationSettings,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Settings
getSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetSettings',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.Settings,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_Settings,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_Settings,
  },
  resetSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/ResetSettings',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setAllowLan: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetAllowLan',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setShowBetaReleases: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetShowBetaReleases',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setLockdownMode: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetLockdownMode',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setAutoConnect: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetAutoConnect',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setWireguardMtu: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWireguardMtu',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.UInt32Value,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_UInt32Value,
    requestDeserialize: deserialize_google_protobuf_UInt32Value,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setWireguardAllowedIps: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWireguardAllowedIps',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.AllowedIpsList,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_AllowedIpsList,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_AllowedIpsList,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setEnableIpv6: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetEnableIpv6',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setQuantumResistantTunnel: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetQuantumResistantTunnel',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.QuantumResistantState,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_QuantumResistantState,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_QuantumResistantState,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setEnableDaita: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetEnableDaita',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setDaitaDirectOnly: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetDaitaDirectOnly',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setDaitaSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetDaitaSettings',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.DaitaSettings,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_DaitaSettings,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_DaitaSettings,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setDnsOptions: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetDnsOptions',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.DnsOptions,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_DnsOptions,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_DnsOptions,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setRelayOverride: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetRelayOverride',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.RelayOverride,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_RelayOverride,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_RelayOverride,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  clearAllRelayOverrides: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearAllRelayOverrides',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setEnableRecents: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetEnableRecents',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setUserspaceWireguard: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetUserspaceWireguard',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Persistent URL of the warren-api server (consumed by
// WarrenRemote{Account,Device}Backend). Format
// `http(s)://host:port` without a trailing slash. Empty string means
// unset (= None on the Settings side, falls back to Mullvad upstream).
// Overridable via the `WARREN_API_URL` env var. A daemon restart is
// required to apply the change.
setWarrenApiUrl: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWarrenApiUrl',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Number of parallel QUIC connections for the Warren tunnel.
// 0 = reset to the compiled default (8). Valid range 1..=16,
// rejected otherwise. Applied on the next (re)connect; the daemon
// reconnects automatically when the tunnel is up. The env var
// `WARREN_N_CONNECTIONS` on the daemon takes priority over this
// setting.
setWarrenNConnections: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWarrenNConnections',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.UInt32Value,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_UInt32Value,
    requestDeserialize: deserialize_google_protobuf_UInt32Value,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Returns the user's BIP39 mnemonic (12 words) so the GUI can let the
// user back it up. Empty string if the identity has never been
// bootstrapped (= legacy Mullvad mode or first boot before
// warren_signer).
// **Sensitive**: the GUI caller must display it with a safety warning
// and explicit user confirmation. The returned string is a
// cryptographic secret, never logged by the daemon (no-log policy).
getWarrenMnemonic: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetWarrenMnemonic',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  // Replaces the user identity with the supplied BIP39 mnemonic.
// **Irreversible**: any subscription tied to the current identity is
// lost. The GUI caller must display a strong confirmation before
// calling. No restart is needed: the daemon hot-swaps the in-memory
// signer (reload_signer_from_disk) and triggers an auto-login so the
// new identity takes effect in the running process. The payload is
// BIP39-validated before being written to disk (= atomic rejection).
setWarrenMnemonic: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWarrenMnemonic',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Signs a community-forum login challenge (DiscourseConnect wallet SSO,
// warren-core doc 55). The GUI passes the `sid` from a
// `warren://forum-login?sid=..` deep link; the daemon signs the fixed
// canonical request `POST /v1/forum/login` with body `{"sid":"<sid>"}`
// using the Warren identity key and returns the four X-Warren-* header
// values. The GUI then POSTs them to the forum connect host. The key
// never leaves the daemon. Errors if no identity is bootstrapped or the
// sid is malformed.
signForumLogin: {
    path: '/mullvad_daemon.management_interface.ManagementService/SignForumLogin',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.ForumLoginRequest,
    responseType: management_interface_pb.ForumLoginSignature,
    requestSerialize: serialize_mullvad_daemon_management_interface_ForumLoginRequest,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_ForumLoginRequest,
    responseSerialize: serialize_mullvad_daemon_management_interface_ForumLoginSignature,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_ForumLoginSignature,
  },
  // Signs a community-forum attach-logs request (warren-core doc 55). The
// GUI passes the `sid` + `topic_id` from a
// `warren://attach-logs?sid=..&topic=..` deep link plus the gzipped
// redacted problem report; the daemon builds the canonical JSON body
// `{"sid":"<sid>","topic_id":<topic>,"log_gz_b64":"<base64>"}`, signs
// `POST /v1/forum/attach-logs` with the Warren identity key, and returns
// the four X-Warren-* header values plus that exact body. The GUI POSTs
// the body verbatim so the signed bytes and the sent bytes are identical.
// The key never leaves the daemon. Errors if no identity is bootstrapped,
// the sid is malformed, or the gzip is empty or exceeds 1 MiB.
signForumAttachLogs: {
    path: '/mullvad_daemon.management_interface.ManagementService/SignForumAttachLogs',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.ForumAttachLogsRequest,
    responseType: management_interface_pb.ForumLoginSignature,
    requestSerialize: serialize_mullvad_daemon_management_interface_ForumAttachLogsRequest,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_ForumAttachLogsRequest,
    responseSerialize: serialize_mullvad_daemon_management_interface_ForumLoginSignature,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_ForumLoginSignature,
  },
  // Warren multi-hop (M4.E.D / two-relayed QUIC HPKE doctrine).
// OFF by default per doctrine `warren_multihop_doctrine_v1` (full
// bandwidth single-hop, opt-in privacy). entry_country / exit_country
// are ISO 3166 alpha-2 codes ("fr", "de", ...); empty string means
// auto-pick.
getWarrenMultiHopSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetWarrenMultiHopSettings',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.WarrenMultiHopSettings,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_WarrenMultiHopSettings,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_WarrenMultiHopSettings,
  },
  setWarrenMultiHopSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWarrenMultiHopSettings',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.WarrenMultiHopSettings,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_WarrenMultiHopSettings,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_WarrenMultiHopSettings,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Advanced "custom exit" override (Settings.warren_custom_exit). When
// enabled with a valid endpoint+pubkey the daemon dials it directly,
// bypassing the signed registry, failover, multi-hop and the TOFU
// pin. Applied on the next (re)connect; the daemon reconnects
// automatically when the tunnel is up. The current value is read back
// through the Settings message (warren_custom_exit field).
setWarrenCustomExit: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWarrenCustomExit',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.WarrenCustomExitSettings,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_WarrenCustomExitSettings,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_WarrenCustomExitSettings,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Warren live tunnel status (reconnect_count + last_reconnect_age
// surface the M4.E.D auto-reconnect supervisor; obfuscation_active
// is always-true for /v1 per `warren_obfuscation_doctrine_v1`).
getWarrenStatus: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetWarrenStatus',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.WarrenStatus,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_WarrenStatus,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_WarrenStatus,
  },
  // Push stream emitting a new WarrenStatus whenever reconnect_count
// changes or the tunnel state machine transitions. The GUI consumes
// this for live UI updates without polling.
warrenStatusUpdates: {
    path: '/mullvad_daemon.management_interface.ManagementService/WarrenStatusUpdates',
    requestStream: false,
    responseStream: true,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.WarrenStatus,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_WarrenStatus,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_WarrenStatus,
  },
  // Session H A.4: TOFU pubkey-pinning user actions. The daemon-side
// verify hook refuses connects when the served Ed25519 pubkey for
// a known `exit_id` differs from the locally pinned baseline; the
// following RPCs let the user resolve the mismatch from the UI
// modal without editing settings.json by hand.
//
// - TrustNewExitKey: replace the pinned key with the newly-observed
//   one and resume connecting. Use case: legitimate key rotation
//   announced by the operator.
// - ResetPinnedExitKeys: clear the entire pin table. Use case: the
//   user switches identity / device and wants a fresh TOFU baseline.
// - DismissPubkeyMismatch: keep the existing pin, clear the
//   pending-mismatch flag from WarrenStatus so the modal unmounts.
//   The daemon stays disconnected; reconnecting would re-trigger
//   the modal until the user picks Trust or Reset.
// - ReportPubkeyMismatch: best-effort POST to
//   `/v1/incidents/pubkey-mismatch`. No PII (cf. the field set).
//   The mismatch flag is cleared regardless of the network outcome.
trustNewExitKey: {
    path: '/mullvad_daemon.management_interface.ManagementService/TrustNewExitKey',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.TrustNewExitKeyRequest,
    responseType: management_interface_pb.TrustNewExitKeyResponse,
    requestSerialize: serialize_mullvad_daemon_management_interface_TrustNewExitKeyRequest,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_TrustNewExitKeyRequest,
    responseSerialize: serialize_mullvad_daemon_management_interface_TrustNewExitKeyResponse,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_TrustNewExitKeyResponse,
  },
  resetPinnedExitKeys: {
    path: '/mullvad_daemon.management_interface.ManagementService/ResetPinnedExitKeys',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.ResetPinnedExitKeysResponse,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_ResetPinnedExitKeysResponse,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_ResetPinnedExitKeysResponse,
  },
  dismissPubkeyMismatch: {
    path: '/mullvad_daemon.management_interface.ManagementService/DismissPubkeyMismatch',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  reportPubkeyMismatch: {
    path: '/mullvad_daemon.management_interface.ManagementService/ReportPubkeyMismatch',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.ReportPubkeyMismatchRequest,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_ReportPubkeyMismatchRequest,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_ReportPubkeyMismatchRequest,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Warren NAT-PMP port-forwarding (RFC 6886). Warren's product
// differentiator since Mullvad / IVPN dropped port-forwarding in
// 2023. OFF by default; when ON the daemon spawns a refresh loop
// against the exit's NAT-PMP server (UDP/5351 of the tunnel
// gateway) and surfaces the granted public port to the UI through
// the NatPmpStatusUpdates stream.
getNatPmpSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetNatPmpSettings',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.NatPmpSettings,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_NatPmpSettings,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_NatPmpSettings,
  },
  setNatPmpSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetNatPmpSettings',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.NatPmpSettings,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_NatPmpSettings,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_NatPmpSettings,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Push stream emitting a new NatPmpStatus on every refresh loop
// event (Mapped / Renewed / Failed / Cancelled).
natPmpStatusUpdates: {
    path: '/mullvad_daemon.management_interface.ManagementService/NatPmpStatusUpdates',
    requestStream: false,
    responseStream: true,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.NatPmpStatus,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_NatPmpStatus,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_NatPmpStatus,
  },
  // Account management
createNewAccount: {
    path: '/mullvad_daemon.management_interface.ManagementService/CreateNewAccount',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  loginAccount: {
    path: '/mullvad_daemon.management_interface.ManagementService/LoginAccount',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  logoutAccount: {
    path: '/mullvad_daemon.management_interface.ManagementService/LogoutAccount',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getAccountData: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetAccountData',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: management_interface_pb.AccountData,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_mullvad_daemon_management_interface_AccountData,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_AccountData,
  },
  getAccountHistory: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetAccountHistory',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.AccountHistory,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_AccountHistory,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_AccountHistory,
  },
  clearAccountHistory: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearAccountHistory',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getWwwAuthToken: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetWwwAuthToken',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  submitVoucher: {
    path: '/mullvad_daemon.management_interface.ManagementService/SubmitVoucher',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: management_interface_pb.VoucherSubmission,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_mullvad_daemon_management_interface_VoucherSubmission,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_VoucherSubmission,
  },
  // Android only
deleteAccount: {
    path: '/mullvad_daemon.management_interface.ManagementService/DeleteAccount',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Device management
getDevice: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetDevice',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.DeviceState,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_DeviceState,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_DeviceState,
  },
  updateDevice: {
    path: '/mullvad_daemon.management_interface.ManagementService/UpdateDevice',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  listDevices: {
    path: '/mullvad_daemon.management_interface.ManagementService/ListDevices',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: management_interface_pb.DeviceList,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_mullvad_daemon_management_interface_DeviceList,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_DeviceList,
  },
  removeDevice: {
    path: '/mullvad_daemon.management_interface.ManagementService/RemoveDevice',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.DeviceRemoval,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_DeviceRemoval,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_DeviceRemoval,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // WireGuard key management
setWireguardRotationInterval: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetWireguardRotationInterval',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_duration_pb.Duration,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Duration,
    requestDeserialize: deserialize_google_protobuf_Duration,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  resetWireguardRotationInterval: {
    path: '/mullvad_daemon.management_interface.ManagementService/ResetWireguardRotationInterval',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  rotateWireguardKey: {
    path: '/mullvad_daemon.management_interface.ManagementService/RotateWireguardKey',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getWireguardKey: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetWireguardKey',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.PublicKey,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_PublicKey,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_PublicKey,
  },
  // Custom lists
createCustomList: {
    path: '/mullvad_daemon.management_interface.ManagementService/CreateCustomList',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.NewCustomList,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_mullvad_daemon_management_interface_NewCustomList,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_NewCustomList,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  deleteCustomList: {
    path: '/mullvad_daemon.management_interface.ManagementService/DeleteCustomList',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  updateCustomList: {
    path: '/mullvad_daemon.management_interface.ManagementService/UpdateCustomList',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.CustomList,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_CustomList,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_CustomList,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  clearCustomLists: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearCustomLists',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Access methods
addApiAccessMethod: {
    path: '/mullvad_daemon.management_interface.ManagementService/AddApiAccessMethod',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.NewAccessMethodSetting,
    responseType: management_interface_pb.UUID,
    requestSerialize: serialize_mullvad_daemon_management_interface_NewAccessMethodSetting,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_NewAccessMethodSetting,
    responseSerialize: serialize_mullvad_daemon_management_interface_UUID,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_UUID,
  },
  removeApiAccessMethod: {
    path: '/mullvad_daemon.management_interface.ManagementService/RemoveApiAccessMethod',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.UUID,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_UUID,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_UUID,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setApiAccessMethod: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetApiAccessMethod',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.UUID,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_UUID,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_UUID,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  updateApiAccessMethod: {
    path: '/mullvad_daemon.management_interface.ManagementService/UpdateApiAccessMethod',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.AccessMethodSetting,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_AccessMethodSetting,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_AccessMethodSetting,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  clearCustomApiAccessMethods: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearCustomApiAccessMethods',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getCurrentApiAccessMethod: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetCurrentApiAccessMethod',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.AccessMethodSetting,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_AccessMethodSetting,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_AccessMethodSetting,
  },
  testCustomApiAccessMethod: {
    path: '/mullvad_daemon.management_interface.ManagementService/TestCustomApiAccessMethod',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.CustomProxy,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_mullvad_daemon_management_interface_CustomProxy,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_CustomProxy,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  testApiAccessMethodById: {
    path: '/mullvad_daemon.management_interface.ManagementService/TestApiAccessMethodById',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.UUID,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_mullvad_daemon_management_interface_UUID,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_UUID,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  // Bridges (Used for reaching the API)
getBridges: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetBridges',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.BridgeList,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_BridgeList,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_BridgeList,
  },
  // Split tunneling (Linux)
getSplitTunnelProcesses: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetSplitTunnelProcesses',
    requestStream: false,
    responseStream: true,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.Int32Value,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Int32Value,
    responseDeserialize: deserialize_google_protobuf_Int32Value,
  },
  addSplitTunnelProcess: {
    path: '/mullvad_daemon.management_interface.ManagementService/AddSplitTunnelProcess',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.Int32Value,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Int32Value,
    requestDeserialize: deserialize_google_protobuf_Int32Value,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  removeSplitTunnelProcess: {
    path: '/mullvad_daemon.management_interface.ManagementService/RemoveSplitTunnelProcess',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.Int32Value,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Int32Value,
    requestDeserialize: deserialize_google_protobuf_Int32Value,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  clearSplitTunnelProcesses: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearSplitTunnelProcesses',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Split tunneling (Linux, Windows)
splitTunnelIsSupported: {
    path: '/mullvad_daemon.management_interface.ManagementService/SplitTunnelIsSupported',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  // Split tunneling (Windows, macOS, Android)
addSplitTunnelApp: {
    path: '/mullvad_daemon.management_interface.ManagementService/AddSplitTunnelApp',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  removeSplitTunnelApp: {
    path: '/mullvad_daemon.management_interface.ManagementService/RemoveSplitTunnelApp',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  setSplitTunnelState: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetSplitTunnelState',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.BoolValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_BoolValue,
    requestDeserialize: deserialize_google_protobuf_BoolValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Split tunneling (Windows, macOS)
clearSplitTunnelApps: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearSplitTunnelApps',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getExcludedProcesses: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetExcludedProcesses',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.ExcludedProcessList,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_ExcludedProcessList,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_ExcludedProcessList,
  },
  // Play payment (Android)
initPlayPurchase: {
    path: '/mullvad_daemon.management_interface.ManagementService/InitPlayPurchase',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.PlayExternalObfuscatedAccountId,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_PlayExternalObfuscatedAccountId,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_PlayExternalObfuscatedAccountId,
  },
  verifyPlayPurchase: {
    path: '/mullvad_daemon.management_interface.ManagementService/VerifyPlayPurchase',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.PlayPurchase,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_PlayPurchase,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_PlayPurchase,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Check whether the app needs TCC approval for split tunneling (macOS)
needFullDiskPermissions: {
    path: '/mullvad_daemon.management_interface.ManagementService/NeedFullDiskPermissions',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.BoolValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_BoolValue,
    responseDeserialize: deserialize_google_protobuf_BoolValue,
  },
  // Notify the split tunnel monitor that a volume was mounted or dismounted
// (Windows).
checkVolumes: {
    path: '/mullvad_daemon.management_interface.ManagementService/CheckVolumes',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Apply a JSON blob to the settings
// See ../../docs/settings-patch-format.md for a description of the format
applyJsonSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/ApplyJsonSettings',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // Return a JSON blob containing all overridable settings, if there are any
exportJsonSettings: {
    path: '/mullvad_daemon.management_interface.ManagementService/ExportJsonSettings',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  // Get current feature indicators
getFeatureIndicators: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetFeatureIndicators',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.FeatureIndicators,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_FeatureIndicators,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_FeatureIndicators,
  },
  // Debug features
disableRelay: {
    path: '/mullvad_daemon.management_interface.ManagementService/DisableRelay',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  enableRelay: {
    path: '/mullvad_daemon.management_interface.ManagementService/EnableRelay',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_wrappers_pb.StringValue,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_StringValue,
    requestDeserialize: deserialize_google_protobuf_StringValue,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  getRolloutThreshold: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetRolloutThreshold',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.Rollout,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_Rollout,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_Rollout,
  },
  regenerateRolloutThreshold: {
    path: '/mullvad_daemon.management_interface.ManagementService/RegenerateRolloutThreshold',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.Rollout,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_Rollout,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_Rollout,
  },
  setRolloutThresholdSeed: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetRolloutThresholdSeed',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.Seed,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_Seed,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_Seed,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  // App upgrade
appUpgrade: {
    path: '/mullvad_daemon.management_interface.ManagementService/AppUpgrade',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  appUpgradeAbort: {
    path: '/mullvad_daemon.management_interface.ManagementService/AppUpgradeAbort',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  appUpgradeEventsListen: {
    path: '/mullvad_daemon.management_interface.ManagementService/AppUpgradeEventsListen',
    requestStream: false,
    responseStream: true,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.AppUpgradeEvent,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_AppUpgradeEvent,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_AppUpgradeEvent,
  },
  getAppUpgradeCacheDir: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetAppUpgradeCacheDir',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_wrappers_pb.StringValue,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_StringValue,
    responseDeserialize: deserialize_google_protobuf_StringValue,
  },
  setLogFilter: {
    path: '/mullvad_daemon.management_interface.ManagementService/SetLogFilter',
    requestStream: false,
    responseStream: false,
    requestType: management_interface_pb.LogFilter,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_mullvad_daemon_management_interface_LogFilter,
    requestDeserialize: deserialize_mullvad_daemon_management_interface_LogFilter,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
  logListen: {
    path: '/mullvad_daemon.management_interface.ManagementService/LogListen',
    requestStream: false,
    responseStream: true,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.LogMessage,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_LogMessage,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_LogMessage,
  },
  // The great multihop migration of 2026
//
// If the return value is SplitFilterMigration, a migration took place recently. After
// ClearMigrationMessage has been called, GetMigrationEvent will indefintely return null.
// If the return value is null, there is nothing for the clients to do. The migration has either
// not been run or was already completed.
getMigrationEvent: {
    path: '/mullvad_daemon.management_interface.ManagementService/GetMigrationEvent',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: management_interface_pb.SplitFilterMigration,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_mullvad_daemon_management_interface_SplitFilterMigration,
    responseDeserialize: deserialize_mullvad_daemon_management_interface_SplitFilterMigration,
  },
  // Call this function after *handling* a SplitFilterMigration as returned by GetMigrationEvent to
// mark the migration as completed. This will cause GetMigrationEvent to return a null-value
// indefintely.
clearMigrationMessage: {
    path: '/mullvad_daemon.management_interface.ManagementService/ClearMigrationMessage',
    requestStream: false,
    responseStream: false,
    requestType: google_protobuf_empty_pb.Empty,
    responseType: google_protobuf_empty_pb.Empty,
    requestSerialize: serialize_google_protobuf_Empty,
    requestDeserialize: deserialize_google_protobuf_Empty,
    responseSerialize: serialize_google_protobuf_Empty,
    responseDeserialize: deserialize_google_protobuf_Empty,
  },
};

exports.ManagementServiceClient = grpc.makeGenericClientConstructor(ManagementServiceService, 'ManagementService');
