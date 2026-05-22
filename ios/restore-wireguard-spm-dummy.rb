#!/usr/bin/env ruby
# frozen_string_literal: true

# C.2.X.C option (d) : re-add the `wireguard-apple` local SPM package
# reference as a no-op placeholder. The hypothesis is that the C.4.4
# WG removal also removed the LAST local SPM package_reference, which
# silently switched Xcode 26.4 to a different (and broken)
# SPM-consumer-path build pipeline that fails to find swift-log's
# `Logging.swiftmodule` for WarrenLogging.
#
# This script only re-adds the package_reference (PBXFileReference
# pointing at `wireguard-apple/Package.swift`). It does NOT re-add the
# packageProductDependencies or Embed Frameworks entries that caused
# the original duplicate-task linker error.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/restore-wireguard-spm-dummy.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

# Check if a wireguard-apple package_reference already exists.
existing = project.root_object.package_references.find do |pkg|
  src = pkg.respond_to?(:repositoryURL) ? pkg.repositoryURL : nil
  src ||= pkg.respond_to?(:relative_path) ? pkg.relative_path : nil
  src && src.to_s.include?("wireguard-apple")
end

if existing
  puts "wireguard-apple package_reference already present (#{existing.uuid}). No changes."
  exit 0
end

# Create a local SPM package reference (XCLocalSwiftPackageReference).
local_ref = project.new(Xcodeproj::Project::Object::XCLocalSwiftPackageReference)
local_ref.relative_path = "wireguard-apple"
project.root_object.package_references << local_ref

project.save
puts "Re-added wireguard-apple as local SPM package_reference (UUID #{local_ref.uuid})."
puts "No package products consumed ; this is a placeholder to keep the SPM build pipeline active."
