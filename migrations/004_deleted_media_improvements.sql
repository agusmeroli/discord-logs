ALTER TABLE messages 
  RENAME COLUMN attachments TO stickers;

ALTER TABLE messages 
  ADD COLUMN attachments TEXT;