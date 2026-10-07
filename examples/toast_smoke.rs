//! 冒烟:走应用自身的 toast 代码路径发一条通知,并用通知历史自检.

fn main() {
    quotakeeper::ui::notify::toast("QuotaKeeper", "smoke: ui::notify::toast 通道验证");
    // toast 在后台线程发送,给它一点时间
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("toast 已发送(检查屏幕右下角与通知中心)");
}
