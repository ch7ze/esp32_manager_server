const fs = require('fs-extra');
const path = require('path');
const crypto = require('crypto');
const terser = require('terser');
const CleanCSS = require('clean-css');
const { exec } = require('child_process');

// Define source and destination directories
const clientDir = path.join(__dirname, '../client');
const destDir = path.join(__dirname, '../dest');

// Compile TypeScript with detailed error reporting
async function compileTypeScript() {
  console.log('Compiling TypeScript...');
  
  return new Promise((resolve, reject) => {
    const { spawn } = require('child_process');
    const tsc = spawn('npx', ['tsc', '--pretty'], { 
      stdio: 'inherit',
      shell: true 
    });
    
    tsc.on('close', (code) => {
      if (code !== 0) {
        console.error('\nTypeScript compilation failed with errors');
        reject(new Error(`TypeScript compiler exited with code ${code}`));
        return;
      }
      console.log('TypeScript compilation successful');
      resolve();
    });
  });
}

// Get all files recursively
function getAllFiles(dir) {
  let files = [];
  const items = fs.readdirSync(dir);
  
  for (const item of items) {
    const fullPath = path.join(dir, item);
    if (fs.statSync(fullPath).isDirectory()) {
      files = files.concat(getAllFiles(fullPath));
    } else {
      files.push(fullPath);
    }
  }
  return files;
}

// Function to calculate hash of directory contents
async function calculateDirectoryHash(directory) {
  // Create a hash object
  const hash = crypto.createHash('md5');

  // Get all files from directory
  const files = getAllFiles(directory).sort(); // Sort to ensure consistent hash
  
  // Update hash with each file's content
  for (const file of files) {
    const relativePath = path.relative(clientDir, file);
    const content = fs.readFileSync(file);
    hash.update(`${relativePath}:${content}`);
  }

  return hash.digest('hex').substring(0, 8);
}

// Minify JavaScript and CSS assets
async function minifyAssets() {
  // Find and minify all JS files
  const jsFiles = getAllFiles(destDir).filter(file => file.endsWith('.js'));
  for (const file of jsFiles) {
    const content = fs.readFileSync(file, 'utf8');
    const minified = await terser.minify(content);
    if (minified.error) {
      console.error(`Error minifying ${file}:`, minified.error);
    } else {
      fs.writeFileSync(file, minified.code);
    }
  }
  
  // Find and minify all CSS files
  const cssFiles = getAllFiles(destDir).filter(file => file.endsWith('.css'));
  for (const file of cssFiles) {
    const content = fs.readFileSync(file, 'utf8');
    const minified = new CleanCSS().minify(content);
    fs.writeFileSync(file, minified.styles);
  }
}

// Main build function
async function build() {
  console.log('Starting build process...');

  // Compile TypeScript first
  await compileTypeScript();

  // Empty destination directory before copying
  fs.emptyDirSync(destDir);

  // Copy all files from client/ to dest/
  fs.copySync(clientDir, destDir);

  // Calculate hash of client directory
  const folderHash = await calculateDirectoryHash(clientDir);
  console.log(`Generated folder hash: ${folderHash}`);

  // Save hash to a config file
  fs.writeFileSync(
    path.join(__dirname, '../client-hash.json'), 
    JSON.stringify({ hash: folderHash })
  );

  // Base tag insertion skipped to preserve SPA functionality
  console.log('Base tag insertion skipped for SPA compatibility');

  // await minifyAssets();
  console.log('Asset minification skipped');

  console.log('Build completed: Files successfully copied to dest/ with hash integration');
}

build().catch(err => console.error('Build error:', err));