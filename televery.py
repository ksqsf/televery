import os
import socket
import threading
import random
from telegram.ext import Updater, CommandHandler


bot = None
trusted_users = ['ksqsf']
chat_id = None
addr = '/tmp/televery'
code_range = (0000, 9999)


def save_chat_id(chat_id):
    with open('chat_id', 'w') as f:
        print('%d' % chat_id, file=f)


def load_chat_id():
    with open('chat_id', 'r') as f:
        return int(f.read())


def start(bot, update):
    """
    Save current chat ID for future verification requests.
    """
    global chat_id
    msg = update.message
    if msg.chat.username in trusted_users:
        chat_id = msg.chat.id
        save_chat_id(chat_id)
        msg.reply_text('OK! Chat ID %d has been saved.' % chat_id)
    else:
        msg.reply_text('Not OK!')


def process_requests():
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    if os.path.exists(addr):
        os.unlink(addr)
    sock.bind(addr)
    sock.listen()
    while True:
        conn, client_addr = sock.accept()
        try:
            code = random.randint(*code_range)
            print('---')
            print('new local request: code=%d' % code)
            print('---')
            bot.send_message(chat_id, 'New verification request, code is %04d' % code)
            conn.sendall(str(code).encode())
        finally:
            conn.close()
    os.unlink(addr)


def main():
    global bot

    # Initialize Bot
    updater = Updater(os.environ['TELEGRAM_BOT_TOKEN'])
    dispatcher = updater.dispatcher

    start_handler = CommandHandler('start', start)
    dispatcher.add_handler(start_handler)

    # Create listener
    t = threading.Thread(target=process_requests)
    t.setDaemon(True)
    t.start()

    # Run
    bot = updater.bot
    updater.start_polling()


if __name__ == '__main__':
    try:
        chat_id = load_chat_id()
    except:
        print('No existing chat ID found, please try /start again.')
    main()
