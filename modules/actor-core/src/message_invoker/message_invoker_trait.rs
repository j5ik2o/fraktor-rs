use crate::{any_message::AnyOwnedMessage, system_message::SystemMessage};

/// メールボックスから取り出したメッセージをアクターへ届けるための抽象。
pub trait MessageInvoker: Send + Sync {
  /// ユーザーメッセージを処理する。
  fn invoke_user_message(&self, message: AnyOwnedMessage);

  /// システムメッセージを処理する。
  fn invoke_system_message(&self, message: SystemMessage);
}
