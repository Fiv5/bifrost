const path = require('path');

const SKILL_ROOT = path.resolve(__dirname, '../../');
const ONCALL_ROOT = path.resolve(__dirname, '../../../../../');

const SCRIPTS_ROOT = path.resolve(__dirname, '../');
const SESSION_DIR = path.join(ONCALL_ROOT, '.env/browser-sessions');
const SCREENSHOT_DIR = path.join(SKILL_ROOT, 'screenshots');
const LOGS_DIR = path.join(SKILL_ROOT, 'logs');
const SCENARIOS_DIR = path.join(SCRIPTS_ROOT, 'scenarios');
const COOKIE_DIR = path.join(ONCALL_ROOT, '.env');
const CONFIG_FILE = path.join(ONCALL_ROOT, '.env/.app_config');

const SHARED_PROFILE_DIR = path.join(SESSION_DIR, 'profile-shared');
const FRONTEND_ROOT = path.join(ONCALL_ROOT, 'oncall-admin');
const ONCALL_ADMIN_SRC = path.join(FRONTEND_ROOT, 'src');

const COOKIES_FILE = 'cookies.json';
const STORAGE_FILE = 'storage.json';

const DEFAULT_PORT = 8000;
const DEFAULT_APP_ID = '7410613250192310283';
const DEFAULT_TIMEOUT = 30000;
const DEFAULT_SESSION = 'default';

const COOKIE_DOMAIN = {
  file: 'nextoncall_bytedance_net',
  domain: 'nextoncall.bytedance.net',
};

module.exports = {
  SKILL_ROOT,
  ONCALL_ROOT,
  SCRIPTS_ROOT,
  SESSION_DIR,
  SCREENSHOT_DIR,
  LOGS_DIR,
  SCENARIOS_DIR,
  COOKIE_DIR,
  CONFIG_FILE,
  SHARED_PROFILE_DIR,
  FRONTEND_ROOT,
  ONCALL_ADMIN_SRC,
  COOKIES_FILE,
  STORAGE_FILE,
  DEFAULT_PORT,
  DEFAULT_APP_ID,
  DEFAULT_TIMEOUT,
  DEFAULT_SESSION,
  COOKIE_DOMAIN,
};
