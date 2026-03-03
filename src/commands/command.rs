pub trait YCommand{
    fn register_command(&self);
    fn get_argment_list(&self)->Vec<String>;
    fn execute(&self);
}
