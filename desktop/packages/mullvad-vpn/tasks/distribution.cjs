const path = require('path');
const fs = require('fs');
const builder = require('electron-builder');
const { Arch } = require('electron-builder');
const { execFileSync } = require('child_process');

const noCompression = process.argv.includes('--no-compression');
const shouldNotarize = process.argv.includes('--notarize');

const universal = process.argv.includes('--universal');
const release = process.argv.includes('--release');

const targets = getOptionValue('--targets');
const hostTargetTriple = getOptionValue('--host-target-triple');

function getOptionValue(option) {
  const optionIndex = process.argv.indexOf(option);
  if (optionIndex !== -1) {
    return process.argv[optionIndex + 1];
  }
}

// Packaging identity per compiled product environment. A non-prod build is a
// separately installable app: own appId/bundle id, own product name (which
// also namespaces the Electron userData dir), own package/executable name,
// own artifact names and its own NSIS upgrade GUID, so it coexists with the
// prod install. A copy of the Rust `warren-product-env` crate's table, like
// src/shared/constants/product-env.ts; the crate's `tests/platform_lockstep.rs`
// reads this file and fails on drift.
const PRODUCT_ENVIRONMENTS = {
  prod: {
    appId: 'com.warrenbrowse.vpn',
    productName: 'Warren VPN',
    packageName: 'warren-vpn',
    artifactPrefix: 'WarrenVPN',
    nsisGuid: '124153A3-C986-44A5-B478-9B263AD94CAD',
    // Per-env URL scheme so a beta and a prod install never fight over
    // deep-link registration (keep in lockstep with
    // src/shared/constants/product-env.ts and the Android manifest
    // placeholder in android/app/build.gradle.kts).
    deepLinkScheme: 'warren',
    // Suffix of the app-icon assets in dist-assets. A non-prod build wears
    // the same artwork with the sand and brown swapped and an amber BETA
    // badge, so a tester can tell which install they are looking at without
    // opening it. Under 128px the badge drops its lettering and stays a
    // plain disc, which still reads at 16px. Staging shares
    // the beta artwork: what matters is being distinguishable from prod, and
    // there is no third palette. Regenerate with
    // desktop/packages/mullvad-vpn/scripts/build-logo-icons.sh.
    iconSuffix: '',
  },
  staging: {
    appId: 'com.warrenbrowse.vpn.staging',
    productName: 'Warren VPN Staging',
    packageName: 'warren-vpn-staging',
    artifactPrefix: 'WarrenVPN-Staging',
    nsisGuid: 'DEB0962E-75E7-4E8F-B6F7-35D40E5CB52E',
    deepLinkScheme: 'warren-staging',
    iconSuffix: '-beta',
  },
  beta: {
    appId: 'com.warrenbrowse.vpn.beta',
    productName: 'Warren VPN Beta',
    packageName: 'warren-vpn-beta',
    artifactPrefix: 'WarrenVPN-Beta',
    nsisGuid: '713AB595-4ABC-47AA-AB0B-9C9F6254016A',
    deepLinkScheme: 'warren-beta',
    iconSuffix: '-beta',
  },
};

const productEnvName = process.env.WARREN_PRODUCT_ENV || 'prod';
const productEnv = PRODUCT_ENVIRONMENTS[productEnvName];
if (!productEnv) {
  throw new Error(`WARREN_PRODUCT_ENV must be prod|staging|beta, got: ${productEnvName}`);
}

// ---------------------------------------------------------------------------
// Per-environment packaging assets. The static assets under dist-assets/
// (systemd units, install scriptlets, macOS pkg-scripts, the AppArmor
// profile, the NSIS include) are written for the prod install. A non-prod
// build is a separately-installable product, so it must not share systemd
// unit names, /usr/bin names, launchd labels, Windows service ids or on-disk
// dirs with prod: the helpers below rewrite the assets at pack time into
// build/env-assets-<env>/ and the packaging config points at the transformed
// copies. Prod uses the originals untouched.
// ---------------------------------------------------------------------------
const envSuffix = productEnvName === 'prod' ? '' : `-${productEnvName}`;

function transformEnvAssetText(text) {
  if (productEnvName === 'prod') {
    return text;
  }
  const spaced = productEnv.productName; // e.g. "Warren VPN Beta"
  const escSpaced = spaced.replaceAll(' ', '\\ ');
  const hexSpaced = spaced.replaceAll(' ', '\\x20');
  const appId = productEnv.appId;
  // Rename INSTALLED names only (unit names, /usr/bin, /usr/local/bin,
  // systemctl invocations, launchd label). Paths INSIDE the packaged app
  // (Contents/Resources/..., /opt/<name>/resources/...) keep their original
  // basenames: the bundle content is not renamed, so rewriting a source path
  // of an `ln -sf`/`cp` would leave a dangling reference.
  return (
    text
      // Display-name paths, in every quoting flavor the assets use.
      .replace(/Warren\\x20VPN(?!\\x20(?:Beta|Staging))/g, hexSpaced)
      .replace(/Warren\\ VPN(?!\\ (?:Beta|Staging))/g, escSpaced)
      .replace(/Warren VPN(?! (?:Beta|Staging))/g, spaced)
      // NetworkManager VPN plugin. The service name is a D-Bus bus name,
      // which has a single owner per machine, so each environment needs its
      // own or the second install could not claim it. Must stay in lockstep
      // with `ProductEnv::nm_vpn_service()`, which the Rust side reads.
      .replace(
        /org\.freedesktop\.NetworkManager\.warren(?![-\w.])/g,
        `org.freedesktop.NetworkManager.warren${envSuffix}`,
      )
      .replace(/^name=warren$/m, `name=warren${envSuffix}`)
      // launchd label + bundle ids.
      .replace(/com\.warrenbrowse\.vpn\.daemon/g, `${appId}.daemon`)
      .replace(/com\.warrenbrowse\.vpn(?![.\w-])/g, appId)
      // Windows SCM service id (case-insensitive; must stay equal to the
      // daemon's compiled service::SERVICE_NAME, e.g. WarrenVPNBeta).
      .replace(/\bwarrenvpn\b/g, `warrenvpn${productEnvName}`)
      // The GUI-was-running marker embeds warren-gui with a suffix of its
      // own, so it is rewritten before the generic warren-gui rule.
      .replace(/warren-gui-was-running/g, `warren-gui${envSuffix}-was-running`)
      // systemd unit names + the systemctl calls that omit .service.
      .replace(/warren-daemon\.service/g, `warren-daemon${envSuffix}.service`)
      .replace(
        /warren-early-boot-blocking\.service/g,
        `warren-early-boot-blocking${envSuffix}.service`,
      )
      .replace(
        /(systemctl (?:status|is-enabled) )warren-daemon(?![-\w])/g,
        `$1warren-daemon${envSuffix}`,
      )
      // Installed binary names.
      .replace(/\/usr\/bin\/warren-daemon(?![-\w])/g, `/usr/bin/warren-daemon${envSuffix}`)
      .replace(/\/usr\/bin\/warren-exclude(?![-\w])/g, `/usr/bin/warren-exclude${envSuffix}`)
      .replace(
        /\/usr\/(local\/)?bin\/warren-problem-report(?![-\w])/g,
        `/usr/$1bin/warren-problem-report${envSuffix}`,
      )
      .replace(/\/usr\/local\/bin\/warren(?![-\w])/g, `/usr/local/bin/warren${envSuffix}`)
      // The renamed GUI executable (see the packLinux afterPack rename):
      // launcher exec, apparmor binary path and the pkill in before-remove.
      .replace(/warren-gui(?![-\w])/g, `warren-gui${envSuffix}`)
      // On-disk product dirs (/var/log/warren-vpn, /var/cache/warren-vpn,
      // /etc/warren-vpn, ...). warren-vpn-daemon-* stays untouched thanks
      // to the lookahead.
      .replace(/warren-vpn(?![-\w])/g, `warren-vpn${envSuffix}`)
      // AppArmor: distinct profile file AND profile name.
      .replace(/\/etc\/apparmor\.d\/warren(?![-\w])/g, `/etc/apparmor.d/warren${envSuffix}`)
      .replace(/^profile warren /m, `profile warren${envSuffix} `)
      // Shell-completion DESTINATIONS only (the sources keep the bundled
      // basenames). The per-env completion files are inert, which is
      // acceptable for a coexisting non-prod install.
      .replace(/\$ZSH_COMPLETIONS_DIR\/_warren/g, `$ZSH_COMPLETIONS_DIR/_warren${envSuffix}`)
      .replace(
        /\$FISH_COMPLETIONS_DIR\/warren\.fish/g,
        `$FISH_COMPLETIONS_DIR/warren${envSuffix}.fish`,
      )
      // The uninstaller spells those same destinations out in full. A link left
      // behind points into a bundle that is gone, and every new zsh then prints
      // a compinit error for it.
      .replace(
        /\/zsh\/site-functions\/_warren(?![-\w])/g,
        `/zsh/site-functions/_warren${envSuffix}`,
      )
      .replace(
        /\/fish\/vendor_completions\.d\/warren\.fish/g,
        `/fish/vendor_completions.d/warren${envSuffix}.fish`,
      )
  );
}

// Returns the path of the (possibly transformed) copy of a dist-assets file.
function envAsset(relativePath) {
  if (productEnvName === 'prod') {
    return distAssets(relativePath);
  }
  const src = distAssets(relativePath);
  const out = buildAssets(path.join(`env-assets-${productEnvName}`, relativePath));
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, transformEnvAssetText(fs.readFileSync(src, 'utf8')), {
    mode: fs.statSync(src).mode,
  });
  return out;
}

// problem-report-link is a committed symlink (fpm packages the link itself),
// so a transformed copy must be a fresh symlink at the per-env target. The
// Linux fpm config is built even on a Windows pack (which then packs only the
// win target), and a git checkout on Windows materializes the symlink as a
// regular file whose content is the link target, so read the target through
// lstat and only recreate a symlink when the source actually is one.
// The NSIS include needs one thing the generic rewrite cannot express: that
// it is packaging a non-production product. The split-tunnel driver is a
// single machine-wide resource, so the installer must skip the driver reset
// rather than fight a production install for it.
function envInstallerScript() {
  const rel = path.join('windows', 'installer.nsh');
  if (productEnvName === 'prod') {
    return distAssets(rel);
  }
  const out = envAsset(rel);
  fs.writeFileSync(out, `!define WARREN_NON_PROD_INSTALL 1\n${fs.readFileSync(out, 'utf8')}`);
  return out;
}

// Windows is the one platform with no per-env CLI name: the exe ships as
// warren.exe in every environment, while macOS and Linux install
// warren-beta / warren-staging. Support instructions therefore could not name
// one command that exists everywhere, and a user given the unix name typed it
// on Windows where nothing answered. A non-prod Windows install ships this
// shim next to the exe so the per-env name works there too; the plain
// warren.exe stays, and prod stays untouched.
function envWindowsCliShim() {
  const out = buildAssets(
    path.join(`env-assets-${productEnvName}`, 'windows', `warren${envSuffix}.cmd`),
  );
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, '@echo off\r\n"%~dp0warren.exe" %*\r\n');
  return out;
}

function envProblemReportLink() {
  if (productEnvName === 'prod') {
    return distAssets('linux/problem-report-link');
  }
  const src = distAssets('linux/problem-report-link');
  const isLink = fs.lstatSync(src).isSymbolicLink();
  const target = (isLink ? fs.readlinkSync(src) : fs.readFileSync(src, 'utf8').trim()).replace(
    'Warren VPN',
    productEnv.productName,
  );
  const out = buildAssets(
    path.join(`env-assets-${productEnvName}`, 'linux', 'problem-report-link'),
  );
  fs.mkdirSync(path.dirname(out), { recursive: true });
  // Staged under a private name and renamed into place. Two packagers can
  // stage this path at once (vitest runs the two specs that load this config
  // in parallel workers), and a remove-then-create pair raced them into
  // EEXIST on the second create; rename(2) replaces whatever sits at `out`
  // atomically, so neither ever meets the other's half-done work.
  // product-env-report-link.spec.ts reproduces the collision.
  const staging = `${out}.${process.pid}`;
  fs.rmSync(staging, { force: true });
  if (isLink) {
    fs.symlinkSync(target, staging);
  } else {
    fs.writeFileSync(staging, target);
  }
  fs.renameSync(staging, out);
  return out;
}

// electron-builder resolves pkg-scripts from directories.buildResources:
// non-prod builds point it at the generated tree holding the transformed
// preinstall/postinstall (every other resource is referenced by absolute
// path, so nothing else needs copying).
function envBuildResources() {
  if (productEnvName === 'prod') {
    return root('dist-assets');
  }
  envAsset(path.join('pkg-scripts', 'preinstall'));
  envAsset(path.join('pkg-scripts', 'postinstall'));
  return buildAssets(`env-assets-${productEnvName}`);
}

function newConfig() {
  return {
    appId: productEnv.appId,
    copyright: 'Warren contributors',
    productName: productEnv.productName,
    publish: null,
    // Register the `warren://` deep-link scheme so the OS routes the
    // community-forum login callback (`warren://forum-login?...`, doc 55) to
    // the app. electron-builder writes CFBundleURLTypes (macOS) and the
    // .desktop MimeType (Linux); Windows registers it at runtime via
    // app.setAsDefaultProtocolClient. See src/main/forum-login.ts.
    protocols: [
      {
        name: 'Warren',
        schemes: [productEnv.deepLinkScheme],
      },
    ],
    asar: true,
    compression: noCompression ? 'store' : 'normal',
    extraResources: [
      { from: distAssets('ca.crt'), to: '.' },
      { from: buildAssets('relays.json'), to: '.' },
      // Warren bootstrap exit list: staged into build/ by build.sh (baked
      // by the CI fetch-warren-relays action, or an inert placeholder).
      // Loaded by the daemon at boot via load_bootstrap as resource_dir/warren-relays.json.
      { from: buildAssets('warren-relays.json'), to: '.' },
      { from: root('CHANGELOG.md'), to: '.' },
    ],

    directories: {
      buildResources: envBuildResources(),
      output: root('dist'),
    },

    extraMetadata: {
      name: productEnv.packageName,
      // We have to stick to semver on Windows for now due to:
      // https://github.com/electron-userland/electron-builder/issues/7173
      version: productVersion(process.platform === 'win32' ? ['semver'] : []),
    },

    files: [
      'package.json',
      'changes.txt',
      'build/',
      '!**/*.tsbuildinfo',
      '!test/',
      '!playwright.config.ts',
      'node_modules/',
      '!node_modules/grpc-tools',
      '!node_modules/@types',
      '!node_modules/@rollup',
      '!node_modules/nseventforwarder/debug',
      '!node_modules/windows-utils/debug',
    ],

    // Make sure that all files declared in "extraResources" exists and abort if they don't.
    afterPack: (context) => {
      if (context.arch !== Arch.universal) {
        const resources = context.packager.platformSpecificBuildOptions.extraResources;
        for (const resource of resources) {
          const filePath = resource.from.replaceAll(
            /\$\{env\.(.*?)\}/g,
            function (match, captureGroup) {
              return process.env[captureGroup];
            },
          );

          if (!fs.existsSync(filePath)) {
            throw new Error(`Can't find file: ${filePath}`);
          }
        }
      }
    },

    mac: {
      target: {
        target: 'pkg',
        arch: getMacArch(),
      },
      x64ArchFiles:
        'Contents/Resources/app.asar.unpacked/node_modules/nseventforwarder/dist/*/index.node',
      artifactName: `${productEnv.artifactPrefix}-\${version}.\${ext}`,
      category: 'public.app-category.tools',
      icon: distAssets(`icon-macos${productEnv.iconSuffix}.icns`),
      notarize: shouldNotarize,
      extendInfo: {
        LSUIElement: true,
        NSUserNotificationAlertStyle: 'banner',
      },
      extraResources: [
        { from: distAssets(path.join('${env.BINARIES_PATH}', 'warren')), to: '.' },
        { from: distAssets(path.join('${env.BINARIES_PATH}', 'warren-problem-report')), to: '.' },
        { from: distAssets(path.join('${env.BINARIES_PATH}', 'warren-daemon')), to: '.' },
        { from: distAssets(path.join('${env.BINARIES_PATH}', 'warren-setup')), to: '.' },
        { from: envAsset('uninstall_macos.sh'), to: './uninstall.sh' },
        { from: buildAssets('shell-completions/_warren'), to: '.' },
        { from: buildAssets('shell-completions/warren.fish'), to: '.' },
      ],
    },

    pkg: {
      allowAnywhere: false,
      allowCurrentUserHome: false,
      isRelocatable: false,
      isVersionChecked: false,
    },

    nsis: {
      guid: productEnv.nsisGuid,
      // Transformed per env: Windows service id, registry keys and
      // display strings must match the compiled daemon and never collide
      // with the prod install.
      oneClick: false,
      perMachine: true,
      allowElevation: true,
      allowToChangeInstallationDirectory: false,
      include: envInstallerScript(),
      installerSidebar: distAssets(`windows/installersidebar${productEnv.iconSuffix}.bmp`),
    },

    win: {
      target: [],
      artifactName: `${productEnv.artifactPrefix}-\${version}_\${arch}.\${ext}`,
      // Explicit: left unset, electron-builder picks up buildResources/icon.ico
      // and a non-prod build would silently ship the prod icon.
      icon: distAssets(`icon${productEnv.iconSuffix}.ico`),
      extraResources: [
        { from: distAssets(path.join('${env.DIST_SUBDIR}', 'warren.exe')), to: '.' },
        ...(productEnvName === 'prod' ? [] : [{ from: envWindowsCliShim(), to: '.' }]),
        {
          from: distAssets(path.join('${env.DIST_SUBDIR}', 'warren-problem-report.exe')),
          to: '.',
        },
        { from: distAssets(path.join('${env.DIST_SUBDIR}', 'warren-daemon.exe')), to: '.' },
        {
          from: distAssets(path.join('${env.DIST_SUBDIR}', 'warren-setup.exe')),
          to: '.',
        },
        {
          from: root(
            path.join(
              'windows',
              'winfw',
              'bin',
              '${env.TARGET_ARCHITECTURE}-${env.CPP_BUILD_MODE}',
              'winfw.dll',
            ),
          ),
          to: '.',
        },
        {
          from: distAssets(path.join('binaries', '${env.TARGET_SUBDIR}', 'wintun/wintun.dll')),
          to: '.',
        },
        {
          from: distAssets(
            path.join('binaries', '${env.TARGET_SUBDIR}', 'split-tunnel/mullvad-split-tunnel.sys'),
          ),
          to: '.',
        },
      ],
    },

    linux: {
      target: [
        {
          target: 'deb',
          arch: getLinuxTargetArch(),
        },
        {
          target: 'rpm',
          arch: getLinuxTargetArch(),
        },
        {
          target: 'pacman',
          arch: getLinuxTargetArch(),
        },
      ],
      executableName: productEnv.packageName,
      artifactName: `${productEnv.artifactPrefix}-\${version}_\${arch}.\${ext}`,
      category: 'Network',
      icon: distAssets(`icon${productEnv.iconSuffix}.icns`),
      extraFiles: [{ from: envAsset('linux/warren-gui-launcher.sh'), to: '.' }],
      extraResources: [
        { from: distAssets(path.join(getLinuxTargetSubdir(), 'warren-problem-report')), to: '.' },
        { from: distAssets(path.join(getLinuxTargetSubdir(), 'warren-setup')), to: '.' },
        { from: envAsset(path.join('linux', 'apparmor_warren')), to: '.' },
      ],
    },

    deb: {
      fpm: [
        '--no-depends',
        '--version',
        getLinuxVersion(),
        '--before-install',
        envAsset('linux/before-install.sh'),
        '--before-remove',
        envAsset('linux/before-remove.sh'),
        envAsset('linux/warren-daemon.service') +
          `=/usr/lib/systemd/system/warren-daemon${envSuffix}.service`,
        envAsset('linux/warren-early-boot-blocking.service') +
          `=/usr/lib/systemd/system/warren-early-boot-blocking${envSuffix}.service`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren')) + `=/usr/bin/warren${envSuffix}`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-daemon')) +
          `=/usr/bin/warren-daemon${envSuffix}`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-exclude')) +
          `=/usr/bin/warren-exclude${envSuffix}`,
        // NetworkManager VPN service plugin: what makes GNOME and KDE show a
        // VPN is up. Inert where NetworkManager is absent, so it adds no
        // package dependency. Not in /usr/bin: nobody runs it by hand,
        // NetworkManager spawns it.
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-nm-vpn-service')) +
          `=/usr/lib/warren-vpn${envSuffix}/warren-nm-vpn-service`,
        envAsset('linux/warren-vpn.name') +
          `=/usr/lib/NetworkManager/VPN/warren-vpn${envSuffix}.name`,
        envAsset('linux/warren-nm-vpn-service.conf') +
          `=/usr/share/dbus-1/system.d/warren-nm-vpn-service${envSuffix}.conf`,
        envProblemReportLink() + `=/usr/bin/warren-problem-report${envSuffix}`,
        buildAssets('shell-completions/warren.bash') +
          `=/usr/share/bash-completion/completions/warren${envSuffix}`,
        buildAssets('shell-completions/_warren') +
          `=/usr/local/share/zsh/site-functions/_warren${envSuffix}`,
        buildAssets('shell-completions/warren.fish') +
          `=/usr/share/fish/vendor_completions.d/warren${envSuffix}.fish`,
      ],
      afterInstall: envAsset('linux/after-install.sh'),
      afterRemove: envAsset('linux/after-remove.sh'),
    },

    rpm: {
      fpm: [
        '--version',
        getLinuxVersion(),
        // Prevents RPM from packaging build-id metadata, some of which is the
        // same across all electron-builder applications, which causes package
        // conflicts
        '--rpm-rpmbuild-define=_build_id_links none',
        `--directories=/opt/${productEnv.productName}/`,
        '--before-install',
        envAsset('linux/before-install.sh'),
        '--before-remove',
        envAsset('linux/before-remove.sh'),
        '--rpm-posttrans',
        envAsset('linux/post-transaction.sh'),
        envAsset('linux/warren-daemon.service') +
          `=/usr/lib/systemd/system/warren-daemon${envSuffix}.service`,
        envAsset('linux/warren-early-boot-blocking.service') +
          `=/usr/lib/systemd/system/warren-early-boot-blocking${envSuffix}.service`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren')) + `=/usr/bin/warren${envSuffix}`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-daemon')) +
          `=/usr/bin/warren-daemon${envSuffix}`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-exclude')) +
          `=/usr/bin/warren-exclude${envSuffix}`,
        // NetworkManager VPN service plugin: what makes GNOME and KDE show a
        // VPN is up. Inert where NetworkManager is absent, so it adds no
        // package dependency. Not in /usr/bin: nobody runs it by hand,
        // NetworkManager spawns it.
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-nm-vpn-service')) +
          `=/usr/lib/warren-vpn${envSuffix}/warren-nm-vpn-service`,
        envAsset('linux/warren-vpn.name') +
          `=/usr/lib/NetworkManager/VPN/warren-vpn${envSuffix}.name`,
        envAsset('linux/warren-nm-vpn-service.conf') +
          `=/usr/share/dbus-1/system.d/warren-nm-vpn-service${envSuffix}.conf`,
        envProblemReportLink() + `=/usr/bin/warren-problem-report${envSuffix}`,
        buildAssets('shell-completions/warren.bash') +
          `=/usr/share/bash-completion/completions/warren${envSuffix}`,
        buildAssets('shell-completions/_warren') +
          `=/usr/share/zsh/site-functions/_warren${envSuffix}`,
        buildAssets('shell-completions/warren.fish') +
          `=/usr/share/fish/vendor_completions.d/warren${envSuffix}.fish`,
      ],
      afterInstall: envAsset('linux/after-install.sh'),
      afterRemove: envAsset('linux/after-remove.sh'),
      depends: ['libXScrnSaver', 'libnotify', 'dbus-libs'],
    },

    // Arch Linux / Arch-based (Manjaro, EndeavourOS...). Built by the same fpm
    // pass as deb/rpm (no recompile, just an extra package write), so it adds
    // only seconds to CI. Same payload as deb: the systemd units, the three
    // binaries, and the shell completions, driven by the same install/remove
    // scriptlets. depends is set explicitly because electron-builder's pacman
    // default list carries packages that no longer exist in the Arch repos
    // (e.g. http-parser) or live only in the AUR (libappindicator-gtk3), which
    // would make `pacman -U` refuse to install.
    pacman: {
      fpm: [
        '--version',
        getPacmanVersion(),
        '--before-install',
        envAsset('linux/before-install.sh'),
        '--before-remove',
        envAsset('linux/before-remove.sh'),
        envAsset('linux/warren-daemon.service') +
          `=/usr/lib/systemd/system/warren-daemon${envSuffix}.service`,
        envAsset('linux/warren-early-boot-blocking.service') +
          `=/usr/lib/systemd/system/warren-early-boot-blocking${envSuffix}.service`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren')) + `=/usr/bin/warren${envSuffix}`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-daemon')) +
          `=/usr/bin/warren-daemon${envSuffix}`,
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-exclude')) +
          `=/usr/bin/warren-exclude${envSuffix}`,
        // NetworkManager VPN service plugin: what makes GNOME and KDE show a
        // VPN is up. Inert where NetworkManager is absent, so it adds no
        // package dependency. Not in /usr/bin: nobody runs it by hand,
        // NetworkManager spawns it.
        distAssets(path.join(getLinuxTargetSubdir(), 'warren-nm-vpn-service')) +
          `=/usr/lib/warren-vpn${envSuffix}/warren-nm-vpn-service`,
        envAsset('linux/warren-vpn.name') +
          `=/usr/lib/NetworkManager/VPN/warren-vpn${envSuffix}.name`,
        envAsset('linux/warren-nm-vpn-service.conf') +
          `=/usr/share/dbus-1/system.d/warren-nm-vpn-service${envSuffix}.conf`,
        envProblemReportLink() + `=/usr/bin/warren-problem-report${envSuffix}`,
        buildAssets('shell-completions/warren.bash') +
          `=/usr/share/bash-completion/completions/warren${envSuffix}`,
        buildAssets('shell-completions/_warren') +
          `=/usr/share/zsh/site-functions/_warren${envSuffix}`,
        buildAssets('shell-completions/warren.fish') +
          `=/usr/share/fish/vendor_completions.d/warren${envSuffix}.fish`,
      ],
      afterInstall: envAsset('linux/after-install.sh'),
      afterRemove: envAsset('linux/after-remove.sh'),
      depends: ['gtk3', 'nss', 'libxss', 'libnotify', 'dbus'],
    },
  };
}

async function packWin() {
  const DEFAULT_ARCH = targets === 'aarch64-pc-windows-msvc' ? 'arm64' : 'x64';

  function prepareWinConfig(arch) {
    const config = newConfig();
    return {
      ...config,
      // The NSIS script resolves ${BUILD_RESOURCES_DIR}\... against dist-assets
      // (wintun.dll, warren-setup.exe, and ..\windows\{nsis-plugins,winfw}), so
      // buildResources must stay dist-assets even for a non-prod build. The
      // per-env transform only swaps the installer.nsh include, referenced by
      // its absolute path in config.nsis.include, independent of this.
      directories: {
        ...config.directories,
        buildResources: root('dist-assets'),
      },
      win: {
        ...config.win,
        target: [
          {
            target: 'nsis',
            arch: arch,
          },
        ],
      },
      asarUnpack: ['build/assets/images/menubar-icons/win32/lock-*.ico', '**/*.node'],
      beforeBuild: (options) => {
        process.env.CPP_BUILD_MODE = release ? 'Release' : 'Debug';
        process.env.CPP_BUILD_TARGET = options.arch;
        process.env.TARGET_ARCHITECTURE = options.arch;
        switch (options.arch) {
          case 'x64':
            process.env.TARGET_TRIPLE = 'x86_64-pc-windows-msvc';
            process.env.SETUP_SUBDIR = '.';
            process.env.TARGET_SUBDIR = 'x86_64-pc-windows-msvc';
            process.env.DIST_SUBDIR = '';

            execFileSync('npm', ['-w', 'windows-utils', 'run', 'build-x86'], { shell: true });
            break;
          case 'arm64':
            process.env.TARGET_TRIPLE = 'aarch64-pc-windows-msvc';
            process.env.SETUP_SUBDIR = 'aarch64-pc-windows-msvc';
            process.env.TARGET_SUBDIR = 'aarch64-pc-windows-msvc';
            process.env.DIST_SUBDIR = 'aarch64-pc-windows-msvc';

            execFileSync('npm', ['-w', 'windows-utils', 'run', 'build-arm'], { shell: true });
            break;
          default:
            throw new Error('Invalid or unknown target (only one may be specified)');
        }
        return true;
      },
      afterAllArtifactBuild: (buildResult) => {
        // All of this is a hack to work around the limitation in:
        // https://github.com/electron-userland/electron-builder/issues/7173
        const productSemverVersion = productVersion(['semver']);
        const productTargetVersion = productVersion([]);

        // Rename the artifacts so that they don't have the .0 (semver format)
        for (const artifactPath of buildResult.artifactPaths) {
          const artifactDir = path.dirname(artifactPath);
          const artifactSemverFilename = path.basename(artifactPath);
          const artifactDesiredFilename = artifactSemverFilename.replace(
            productSemverVersion,
            productTargetVersion,
          );
          const targetArtifactPath = path.join(artifactDir, artifactDesiredFilename);
          console.log('Moving', artifactSemverFilename, '=>', artifactDesiredFilename);
          fs.renameSync(artifactPath, targetArtifactPath);
        }
      },
    };
  }

  if (universal) {
    // For universal builds, we simply build for all targets. It is up to build.sh to pack the
    // installers in the same binary.
    await builder.build({
      targets: builder.Platform.WINDOWS.createTarget(),
      config: prepareWinConfig(DEFAULT_ARCH === 'x64' ? 'arm64' : 'x64'),
    });
  }

  return builder.build({
    targets: builder.Platform.WINDOWS.createTarget(),
    config: prepareWinConfig(DEFAULT_ARCH),
  });
}

function packMac() {
  const appOutDirs = [];
  const config = newConfig();

  // A universal app is merged from an x64 and an arm64 slice, and
  // @electron/universal aborts the merge as soon as a file exists in one slice
  // and not the other. nseventforwarder is a per-arch native module
  // (dist/darwin-x64 vs dist/darwin-arm64) that beforeBuild compiles lazily,
  // one arch at a time, while electron-builder packs x64 FIRST: the x64 slice
  // then holds one .node and the arm64 slice two, and the merge dies on
  // "the number of mach-o files is not the same". mac.x64ArchFiles only excuses
  // files unique to x64, so it cannot cover that direction.
  //
  // Building both up front makes every slice carry both modules, which is the
  // shape x64ArchFiles expects. The per-arch beforeBuild calls below stay: they
  // also set TARGET_TRIPLE/BINARIES_PATH, and cargo makes the rebuild a no-op.
  if (universal) {
    execFileSync('npm', ['-w', 'nseventforwarder', 'run', 'build-x86']);
    execFileSync('npm', ['-w', 'nseventforwarder', 'run', 'build-arm']);
  }

  return builder.build({
    targets: builder.Platform.MAC.createTarget(),
    config: {
      ...config,
      asarUnpack: ['**/*.node'],
      beforeBuild: async (options) => {
        switch (options.arch) {
          case 'x64':
            process.env.TARGET_TRIPLE = 'x86_64-apple-darwin';
            execFileSync('npm', ['-w', 'nseventforwarder', 'run', 'build-x86']);
            break;
          case 'arm64':
            process.env.TARGET_TRIPLE = 'aarch64-apple-darwin';
            execFileSync('npm', ['-w', 'nseventforwarder', 'run', 'build-arm']);
            break;
          default:
            delete process.env.TARGET_TRIPLE;
            break;
        }

        process.env.BINARIES_PATH =
          hostTargetTriple !== process.env.TARGET_TRIPLE ? process.env.TARGET_TRIPLE : '';

        return true;
      },
      beforePack: async (context) => {
        if (!universal) {
          // Ensure we don't pack native modules for other architectures.
          // These will exist if the app has been built for other architectures before.
          await removeNseventforwarderNativeModules();
        }
        config.beforePack?.(context);
      },
      afterPack: (context) => {
        config.afterPack?.(context);

        if (context.arch !== Arch.universal) {
          delete process.env.TARGET_TRIPLE;
          appOutDirs.push(context.appOutDir);
        }

        return Promise.resolve();
      },
      afterAllArtifactBuild: async (_buildResult) => {
        // Remove the folder that contains the unpacked app. Electron builder cleans up some of
        // these directories and it's changed between versions without a mention in the changelog.
        for (const dir of appOutDirs) {
          try {
            await fs.promises.rm(dir, { recursive: true });
          } catch {
            // noop
          }
        }
      },
      afterSign: (context) => {
        const appOutDir = context.appOutDir;
        appOutDirs.push(appOutDir);
      },
    },
  });
}

function packLinux() {
  const config = newConfig();

  if (noCompression) {
    config.rpm.fpm.unshift('--rpm-compression', 'none');
    config.pacman.fpm.unshift('--pacman-compression', 'none');
  }

  if (targets && targets === 'aarch64-unknown-linux-gnu') {
    config.rpm.fpm.unshift('--architecture', 'aarch64');
    config.pacman.fpm.unshift('--architecture', 'aarch64');
  }

  return builder.build({
    targets: builder.Platform.LINUX.createTarget(),
    config: {
      ...config,
      beforeBuild: (options) => {
        switch (options.arch) {
          case 'x64':
            process.env.TARGET_TRIPLE = 'x86_64-unknown-linux-gnu';
            break;
          case 'arm64':
            process.env.TARGET_TRIPLE = 'aarch64-unknown-linux-gnu';
            break;
          default:
            delete process.env.TARGET_TRIPLE;
            break;
        }

        return true;
      },
      afterPack: async (context) => {
        config.afterPack?.(context);

        const sourceExecutable = path.join(context.appOutDir, productEnv.packageName);
        const targetExecutable = path.join(context.appOutDir, `warren-gui${envSuffix}`);
        const launcherScript = path.join(context.appOutDir, 'warren-gui-launcher.sh');

        // rename the packaged executable to warren-gui
        await fs.promises.rename(sourceExecutable, targetExecutable);
        // the launcher script takes the executable's name
        await fs.promises.rename(launcherScript, sourceExecutable);
      },
    },
  });
}

function buildAssets(relativePath) {
  return root(path.join('build', relativePath));
}

function distAssets(relativePath) {
  return root(path.join('dist-assets', relativePath));
}

function root(relativePath) {
  return path.join(path.resolve(__dirname, '../../../../'), relativePath);
}

function getLinuxTargetArch() {
  if (targets && process.platform === 'linux') {
    if (targets === 'aarch64-unknown-linux-gnu') {
      return 'arm64';
    }
    throw new Error('Invalid or unknown target (only one may be specified)');
  }
  // Use host architecture.
  return undefined;
}

function getLinuxTargetSubdir() {
  if (targets && process.platform === 'linux') {
    if (targets === 'aarch64-unknown-linux-gnu') {
      return targets;
    }
    throw new Error('Invalid or unknown target (only one may be specified)');
  }
  return '';
}

function getMacArch() {
  if (universal) {
    return 'universal';
  } else {
    // Not specifying an arch makes Electron builder build for the arch it's running on.
    return undefined;
  }
}

// Replace '-' with `~` (tilde) before the beta component, to make the version comparison
// understand that stable `YYYY.NN` is newer than beta `YYYY.NN-betaN`. Both Debian and
// Fedora do this where a tilde denotes a version component that must be sorted as earlier
// than a non-tilde version component
// https://docs.fedoraproject.org/en-US/packaging-guidelines/Versioning/#_complex_versioning
// Arch's pkgver forbids the hyphen (it separates pkgver from pkgrel). Clean
// release tags produce a hyphen-free version already, but a local dev build
// carries "-dev-<sha>", which fpm would reject and fail the whole Linux pack
// (deb/rpm included). Map any remaining hyphen to an underscore so pacman
// packaging never breaks a dev build.
function getPacmanVersion() {
  return getLinuxVersion().replace(/-/g, '_');
}

// The whole version reaches the package, patch component included. Upstream
// Mullvad numbers releases `YYYY.NN`, so keeping only major.minor was lossless
// there; Warren is semver, and truncating it stamped every Linux package
// `0.0`. dpkg and rpm then see no version change between two releases, so
// `apt install ./WarrenVPN-*.deb` answers "already the newest version" and no
// package manager can ever upgrade the app.
function getLinuxVersion() {
  const [version, ...prereleaseParts] = productVersion([]).split('-');
  const prerelease = prereleaseParts.join('-');
  if (prerelease) {
    // A tilde sorts before anything in both dpkg and rpm, which is what makes
    // stable 1.2.3 newer than its own 1.2.3-beta1.
    if (prerelease.toLowerCase().startsWith('beta')) {
      return `${version}~${prerelease}`;
    }
    return `${version}-${prerelease}`;
  }
  return version;
}

// Returns the product version. The `args` argument is optional. Set it to `'semver'`
// to get the version in semver format.
function productVersion(extraArgs) {
  const args = ['run', '-q', '--bin', 'mullvad-version', ...extraArgs];
  return execFileSync('cargo', args, { encoding: 'utf-8' }).trim();
}

async function removeNseventforwarderNativeModules() {
  try {
    await fs.promises.rm('../../node_modules/nseventforwarder/dist/', { recursive: true });
  } catch {
    // noop
  }
}

exports.newConfig = newConfig;
exports.envProblemReportLink = envProblemReportLink;
exports.packWin = packWin;
exports.packMac = packMac;
exports.packLinux = packLinux;
