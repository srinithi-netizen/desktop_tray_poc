
const http = require('http')
const fs = require('fs')
const path = require('path')
const os = require('os')
const UPLOAD_DIR = path.join(os.homedir(), 'FluxBooks-Server-Uploads')
const CHUNKS_DIR = path.join(UPLOAD_DIR, 'chunks')
const FILES_DIR  = path.join(UPLOAD_DIR, 'files')

fs.mkdirSync(CHUNKS_DIR, { recursive: true })
fs.mkdirSync(FILES_DIR,  { recursive: true })

console.log(`📁 Chunks folder : ${CHUNKS_DIR}`)
console.log(`📁 Files folder  : ${FILES_DIR}`)

const server = http.createServer((req, res) => {
  const uploadId    = req.headers['x-upload-id']
  const fileName    = req.headers['x-file-name']
  const chunkIndex  = parseInt(req.headers['x-chunk-index'])
  const totalChunks = parseInt(req.headers['x-total-chunks'])

  console.log('\n─────────────────────────────────────────')
  console.log(`File     : ${fileName}`)
  console.log(`Upload ID: ${uploadId}`)
  console.log(`Chunk    : ${chunkIndex + 1} of ${totalChunks}`)

  let body = []
  req.on('data', chunk => body.push(chunk))

  req.on('end', async () => {
    try {
      const chunkBytes = Buffer.concat(body)
      console.log(`Bytes    : ${chunkBytes.length}`)

      // Save this chunk to disk
      const chunkPath = path.join(
        CHUNKS_DIR,
        `${uploadId}_chunk_${chunkIndex}.bin`
      )
      fs.writeFileSync(chunkPath, chunkBytes)
      console.log(`Saved    : ${chunkPath}`)

      // Check if ALL chunks have arrived
      const allChunksExist = checkAllChunks(uploadId, totalChunks)

      if (allChunksExist) {
        console.log(`\n✅ All ${totalChunks} chunks received — reassembling...`)

        const finalPath = path.join(FILES_DIR, `${uploadId}_${fileName}`)

        // Wait for reassembly to fully finish before checking size
        await reassembleFile(uploadId, totalChunks, finalPath)

        const size = fs.statSync(finalPath).size
        console.log(`✅ File saved : ${finalPath}`)
        console.log(`✅ Final size : ${size} bytes`)

        // Clean up chunk files
        for (let i = 0; i < totalChunks; i++) {
          const cp = path.join(CHUNKS_DIR, `${uploadId}_chunk_${i}.bin`)
          if (fs.existsSync(cp)) fs.unlinkSync(cp)
        }
        console.log(`🧹 Chunks cleaned up`)
      }

      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ success: true }))

    } catch (err) {
      console.error('Error processing chunk:', err)
      res.writeHead(500, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ success: false, error: err.message }))
    }
  })

  req.on('error', (err) => {
    console.error('Request error:', err)
    res.writeHead(500)
    res.end(JSON.stringify({ success: false }))
  })
})

// Check if all chunk files exist on disk
function checkAllChunks(uploadId, totalChunks) {
  for (let i = 0; i < totalChunks; i++) {
    const chunkPath = path.join(CHUNKS_DIR, `${uploadId}_chunk_${i}.bin`)
    if (!fs.existsSync(chunkPath)) return false
  }
  return true
}

// Join chunks in order — returns Promise so we can await it
function reassembleFile(uploadId, totalChunks, finalPath) {
  return new Promise((resolve, reject) => {
    const writeStream = fs.createWriteStream(finalPath)

    writeStream.on('finish', resolve)
    writeStream.on('error', reject)

    for (let i = 0; i < totalChunks; i++) {
      const chunkPath = path.join(CHUNKS_DIR, `${uploadId}_chunk_${i}.bin`)
      const chunkData = fs.readFileSync(chunkPath)
      writeStream.write(chunkData)
      console.log(`  Joined chunk ${i + 1}/${totalChunks} (${chunkData.length} bytes)`)
    }

    writeStream.end()
  })
}

server.listen(3000, '127.0.0.1', () => {
  console.log('\n✅ FluxBooks Upload Server running at http://localhost:3000')
  console.log('Waiting for uploads...\n')
})