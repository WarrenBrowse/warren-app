// Stand-in for the `electron` module in the unit suite.
//
// `desktop/.npmrc` sets `ignore-scripts=true`, so the Electron binary is never
// downloaded by an install and `require('electron')` throws "Electron failed to
// install correctly". Half the main-process modules pull it in through their
// import graph (logging, forum-login, app-upgrade), which took six spec files
// down with it. None of them touch an Electron API in the code under test, so
// an empty module is all they need, and the suite stays runnable without a
// 100 MB GUI binary.
module.exports = {};
