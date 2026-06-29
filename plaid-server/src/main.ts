import { NestFactory } from '@nestjs/core';
import { AppModule } from './app.module';

async function bootstrap() {
  const app = await NestFactory.create(AppModule);

  // Allow Tauri app and browser to call this server
  app.enableCors({
    origin: ['http://localhost:1420', 'http://localhost:5173', 'tauri://localhost'],
    methods: ['GET', 'POST'],
  });

  const port = process.env.PORT || 3001;
  await app.listen(port);
  console.log(`✅ Plaid server running on http://localhost:${port}`);
}
bootstrap();